//! Fragmented MP4 (ISO/IEC 14496-12 §8.8) parsing and sample table synthesis.
//!
//! Handles `mvex`/`trex`, `moof`/`mfhd`/`traf`/`tfhd`/`tfdt`/`trun`, and builds
//! a unified `SampleTable` for fragmented movie tracks.

use alloc::vec::Vec;
use sillstone_digest::Result;

use crate::boxes::{Boxes, Reader, full_box};
use crate::sample_table::{ChunkRun, CompRun, SampleTable, TimeRun};

/// Track extends defaults from `trex` box under `mvex`.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrexDefaults {
    /// Track ID.
    pub track_id: u32,
    /// Default sample description index.
    pub default_sample_description_index: u32,
    /// Default sample duration.
    pub default_sample_duration: u32,
    /// Default sample size.
    pub default_sample_size: u32,
    /// Default sample flags.
    pub default_sample_flags: u32,
}

/// Parses the `mvex` box and collects `trex` defaults.
pub fn parse_mvex(data: &[u8]) -> Result<Vec<TrexDefaults>> {
    let mut defaults = Vec::new();
    for head in Boxes::new(data, 0..data.len()) {
        let head = head?;
        if head.four == *b"trex" {
            let mut r = Reader::new(&data[head.payload]);
            full_box(&mut r, "trex")?;
            let track_id = r.u32("trex track_ID")?;
            let default_sample_description_index = r.u32("trex default_sample_description_index")?;
            let default_sample_duration = r.u32("trex default_sample_duration")?;
            let default_sample_size = r.u32("trex default_sample_size")?;
            let default_sample_flags = r.u32("trex default_sample_flags")?;
            defaults.push(TrexDefaults {
                track_id,
                default_sample_description_index,
                default_sample_duration,
                default_sample_size,
                default_sample_flags,
            });
        }
    }
    Ok(defaults)
}

/// One parsed fragmented sample.
#[derive(Clone, Copy, Debug)]
pub struct FragSample {
    /// Absolute offset in the file.
    pub offset: u64,
    /// Sample size in bytes.
    pub size: u32,
    /// Sample duration in timescale ticks.
    pub duration: u32,
    /// Signed composition time offset.
    pub comp_offset: i32,
    /// True if sync/keyframe.
    pub keyframe: bool,
}

/// Parses all `moof` boxes and groups their samples by `track_id`.
pub fn parse_moof_fragments(
    data: &[u8],
    moof_ranges: &[core::ops::Range<usize>],
    trex_defaults: &[TrexDefaults],
) -> Result<alloc::collections::BTreeMap<u32, Vec<FragSample>>> {
    use alloc::collections::BTreeMap;
    let mut track_samples: BTreeMap<u32, Vec<FragSample>> = BTreeMap::new();

    for moof_range in moof_ranges {
        let moof_offset = moof_range.start as u64;
        let moof_bytes = &data[moof_range.clone()];
        for head in Boxes::new(moof_bytes, 0..moof_bytes.len()) {
            let head = head?;
            if head.four == *b"traf" {
                parse_traf(
                    data,
                    moof_offset,
                    head.offset as u64 + moof_offset,
                    &moof_bytes[head.payload],
                    trex_defaults,
                    &mut track_samples,
                )?;
            }
        }
    }

    Ok(track_samples)
}

fn parse_traf(
    _data: &[u8],
    moof_start_offset: u64,
    _traf_offset: u64,
    traf_bytes: &[u8],
    trex_defaults: &[TrexDefaults],
    track_samples: &mut alloc::collections::BTreeMap<u32, Vec<FragSample>>,
) -> Result<()> {
    let mut tfhd: Option<Tfhd> = None;
    let mut _tfdt_decode_time: Option<u64> = None;
    let mut trun_boxes = Vec::new();

    for head in Boxes::new(traf_bytes, 0..traf_bytes.len()) {
        let head = head?;
        let body = &traf_bytes[head.payload];
        match &head.four {
            b"tfhd" => {
                tfhd = Some(parse_tfhd(body, moof_start_offset)?);
            }
            b"tfdt" => {
                let mut r = Reader::new(body);
                let (version, _) = full_box(&mut r, "tfdt")?;
                let dt = if version == 0 {
                    r.u32("tfdt baseMediaDecodeTime")? as u64
                } else {
                    r.u64("tfdt baseMediaDecodeTime")?
                };
                _tfdt_decode_time = Some(dt);
            }
            b"trun" => {
                trun_boxes.push(body);
            }
            _ => {}
        }
    }

    let tfhd = match tfhd {
        Some(t) => t,
        None => return Ok(()),
    };

    let trex = trex_defaults
        .iter()
        .find(|t| t.track_id == tfhd.track_id)
        .copied()
        .unwrap_or_default();

    let def_duration = tfhd
        .default_sample_duration
        .unwrap_or(trex.default_sample_duration);
    let def_size = tfhd.default_sample_size.unwrap_or(trex.default_sample_size);
    let def_flags = tfhd
        .default_sample_flags
        .unwrap_or(trex.default_sample_flags);

    let mut current_offset = tfhd.base_data_offset.unwrap_or(moof_start_offset);

    let samples = track_samples.entry(tfhd.track_id).or_default();

    for trun_body in trun_boxes {
        let mut r = Reader::new(trun_body);
        let (version, flags) = full_box(&mut r, "trun")?;
        let sample_count = r.u32("trun sample_count")? as usize;

        let data_offset_present = (flags & 0x000001) != 0;
        let first_sample_flags_present = (flags & 0x000004) != 0;
        let sample_duration_present = (flags & 0x000100) != 0;
        let sample_size_present = (flags & 0x000200) != 0;
        let sample_flags_present = (flags & 0x000400) != 0;
        let sample_comp_present = (flags & 0x000800) != 0;

        let trun_data_offset = if data_offset_present {
            r.i32("trun data_offset")? as i64
        } else {
            0
        };

        let first_sample_flags = if first_sample_flags_present {
            Some(r.u32("trun first_sample_flags")?)
        } else {
            None
        };

        let mut sample_offset = (current_offset as i64 + trun_data_offset) as u64;

        for i in 0..sample_count {
            let duration = if sample_duration_present {
                r.u32("trun sample_duration")?
            } else {
                def_duration
            };

            let size = if sample_size_present {
                r.u32("trun sample_size")?
            } else {
                def_size
            };

            let s_flags = if sample_flags_present {
                r.u32("trun sample_flags")?
            } else if i == 0 && first_sample_flags.is_some() {
                first_sample_flags.unwrap()
            } else {
                def_flags
            };

            let comp_offset = if sample_comp_present {
                if version == 0 {
                    r.u32("trun sample_comp_offset")? as i32
                } else {
                    r.i32("trun sample_comp_offset")?
                }
            } else {
                0
            };

            // Non-sync sample is flag bit 0x01000000 (sample_is_non_sync_sample)
            let is_sync = (s_flags & 0x0100_0000) == 0;

            samples.push(FragSample {
                offset: sample_offset,
                size,
                duration,
                comp_offset,
                keyframe: is_sync,
            });

            sample_offset += size as u64;
        }

        current_offset = sample_offset;
    }

    Ok(())
}

struct Tfhd {
    track_id: u32,
    base_data_offset: Option<u64>,
    default_sample_duration: Option<u32>,
    default_sample_size: Option<u32>,
    default_sample_flags: Option<u32>,
}

fn parse_tfhd(body: &[u8], moof_start_offset: u64) -> Result<Tfhd> {
    let mut r = Reader::new(body);
    let (_version, flags) = full_box(&mut r, "tfhd")?;
    let track_id = r.u32("tfhd track_ID")?;

    let base_data_offset = if (flags & 0x000001) != 0 {
        Some(r.u64("tfhd base_data_offset")?)
    } else if (flags & 0x020000) != 0 {
        // default-base-is-moof
        Some(moof_start_offset)
    } else {
        None
    };

    if (flags & 0x000002) != 0 {
        r.u32("tfhd sample_description_index")?;
    }

    let default_sample_duration = if (flags & 0x000008) != 0 {
        Some(r.u32("tfhd default_sample_duration")?)
    } else {
        None
    };

    let default_sample_size = if (flags & 0x000010) != 0 {
        Some(r.u32("tfhd default_sample_size")?)
    } else {
        None
    };

    let default_sample_flags = if (flags & 0x000020) != 0 {
        Some(r.u32("tfhd default_sample_flags")?)
    } else {
        None
    };

    Ok(Tfhd {
        track_id,
        base_data_offset,
        default_sample_duration,
        default_sample_size,
        default_sample_flags,
    })
}

/// Builds a `SampleTable` from a slice of `FragSample`s and an existing sample description.
pub fn synthesize_sample_table(
    table: &mut SampleTable,
    frag_samples: &[FragSample],
) {
    if frag_samples.is_empty() {
        return;
    }

    table.sample_count = frag_samples.len();
    table.uniform_size = 0;
    table.sizes = frag_samples.iter().map(|s| s.size).collect();
    table.chunk_offsets = frag_samples.iter().map(|s| s.offset).collect();
    table.chunks = vec![ChunkRun {
        first_chunk: 1,
        samples_per_chunk: 1,
        description_index: 1,
    }];

    // Run-length encode times
    let mut times: Vec<TimeRun> = Vec::new();
    for s in frag_samples {
        if let Some(last) = times.last_mut() {
            if last.delta == s.duration {
                last.count += 1;
                continue;
            }
        }
        times.push(TimeRun {
            count: 1,
            delta: s.duration,
        });
    }
    table.times = times;

    // Run-length encode composition times
    let mut comp: Vec<CompRun> = Vec::new();
    let has_comp = frag_samples.iter().any(|s| s.comp_offset != 0);
    if has_comp {
        for s in frag_samples {
            if let Some(last) = comp.last_mut() {
                if last.offset == s.comp_offset {
                    last.count += 1;
                    continue;
                }
            }
            comp.push(CompRun {
                count: 1,
                offset: s.comp_offset,
            });
        }
    }
    table.comp = comp;

    // Sync points
    let all_sync = frag_samples.iter().all(|s| s.keyframe);
    if all_sync {
        table.sync = None;
    } else {
        let sync_indices: Vec<u32> = frag_samples
            .iter()
            .enumerate()
            .filter(|(_, s)| s.keyframe)
            .map(|(i, _)| (i + 1) as u32)
            .collect();
        table.sync = Some(sync_indices);
    }
}

/// Demuxes a fragmented ISO-BMFF stream (`moov` + one or more `moof` boxes).
pub fn demux_fragmented(data: &[u8]) -> Result<crate::Mp4> {
    let mut top = Boxes::new(data, 0..data.len());
    let first = top
        .next()
        .ok_or(sillstone_digest::Error::InvalidMagic { what: "empty file" })??;
    if first.four != *b"ftyp" {
        return Err(sillstone_digest::Error::InvalidMagic {
            what: "first box is not ftyp",
        });
    }
    let (major_brand, compatible_brands) = crate::demux::parse_ftyp(&data[first.payload.clone()])?;
    let mut moov = None;
    let mut moof_ranges = Vec::new();
    for head in top {
        let head = head?;
        match &head.four {
            b"moov" => {
                if moov.is_some() {
                    return Err(sillstone_digest::Error::BadValue("more than one moov box"));
                }
                moov = Some(head.payload);
            }
            b"moof" => {
                moof_ranges.push(head.payload);
            }
            _ => {}
        }
    }
    let moov = moov.ok_or(sillstone_digest::Error::BadValue("no moov box"))?;
    let mut timescale = 0u32;
    let mut duration = 0u64;
    let mut tracks = Vec::new();
    let mut trex_defaults = Vec::new();
    for head in Boxes::new(data, moov) {
        let head = head?;
        match &head.four {
            b"mvhd" => {
                let (ts, dur) = crate::demux::parse_mvhd(&data[head.payload.clone()])?;
                timescale = ts;
                duration = dur;
            }
            b"trak" => tracks.push(crate::demux::parse_trak(data, head.payload)?),
            b"mvex" => {
                trex_defaults = parse_mvex(&data[head.payload])?;
            }
            _ => {}
        }
    }
    if !moof_ranges.is_empty() {
        let frag_map = parse_moof_fragments(data, &moof_ranges, &trex_defaults)?;
        for track in &mut tracks {
            if let Some(samples) = frag_map.get(&track.id) {
                synthesize_sample_table(&mut track.table, samples);
            }
        }
    }
    Ok(crate::Mp4 {
        major_brand,
        compatible_brands,
        timescale,
        duration,
        tracks,
    })
}
