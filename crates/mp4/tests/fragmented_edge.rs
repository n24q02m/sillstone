//! Edge cases for fragmented MP4 demuxing (ISO/IEC 14496-12 §8.8), checked
//! against offsets and flags worked out from the specification text.

use sillstone_digest::Error;
use sillstone_mp4::fragmented::{demux_fragmented, parse_mvex};

fn bx(four: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&((8 + payload.len()) as u32).to_be_bytes());
    b.extend_from_slice(four);
    b.extend_from_slice(payload);
    b
}

fn full(four: &[u8; 4], version: u8, flags: u32, payload: &[u8]) -> Vec<u8> {
    let mut p = vec![version];
    p.extend_from_slice(&flags.to_be_bytes()[1..4]);
    p.extend_from_slice(payload);
    bx(four, &p)
}

fn cat(parts: &[Vec<u8>]) -> Vec<u8> {
    parts.concat()
}

fn ftyp() -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(b"isom");
    p.extend_from_slice(&0x200u32.to_be_bytes());
    p.extend_from_slice(b"isom");
    bx(b"ftyp", &p)
}

/// One empty-`stbl` video track with the given id.
fn trak(id: u32, stbl_children: &[Vec<u8>]) -> Vec<u8> {
    let mut tkhd = vec![0u8; 80];
    tkhd[8..12].copy_from_slice(&id.to_be_bytes());
    tkhd[72..76].copy_from_slice(&(64u32 << 16).to_be_bytes());
    tkhd[76..80].copy_from_slice(&(48u32 << 16).to_be_bytes());
    let mut mdhd = vec![0u8; 20];
    mdhd[8..12].copy_from_slice(&1000u32.to_be_bytes());
    let mut hdlr = vec![0u8; 20];
    hdlr[8..12].copy_from_slice(b"vide");
    let mut stsd = 1u32.to_be_bytes().to_vec();
    let mut entry = vec![0u8; 78];
    entry[24..26].copy_from_slice(&64u16.to_be_bytes());
    entry[26..28].copy_from_slice(&48u16.to_be_bytes());
    stsd.extend_from_slice(&bx(b"avc1", &entry));

    let mut stbl = full(b"stsd", 0, 0, &stsd);
    if stbl_children.is_empty() {
        stbl.extend(full(b"stts", 0, 0, &[0; 4]));
        stbl.extend(full(b"stsc", 0, 0, &[0; 4]));
        stbl.extend(full(b"stsz", 0, 0, &[0; 8]));
        stbl.extend(full(b"stco", 0, 0, &[0; 4]));
    } else {
        stbl.extend(cat(stbl_children));
    }
    let minf = bx(b"minf", &bx(b"stbl", &stbl));
    let mdia = bx(
        b"mdia",
        &cat(&[full(b"mdhd", 0, 0, &mdhd), full(b"hdlr", 0, 0, &hdlr), minf]),
    );
    bx(b"trak", &cat(&[full(b"tkhd", 0, 0, &tkhd), mdia]))
}

fn trex(track: u32, dur: u32, size: u32, flags: u32) -> Vec<u8> {
    let mut p = Vec::new();
    for v in [track, 1, dur, size, flags] {
        p.extend_from_slice(&v.to_be_bytes());
    }
    full(b"trex", 0, 0, &p)
}

fn mvhd() -> Vec<u8> {
    let mut p = vec![0u8; 96];
    p[12..16].copy_from_slice(&1000u32.to_be_bytes());
    p[16..20].copy_from_slice(&4000u32.to_be_bytes());
    full(b"mvhd", 0, 0, &p)
}

/// `ftyp` + `moov` with the given tracks and `trex` boxes.
fn init(traks: &[Vec<u8>], trexes: &[Vec<u8>]) -> Vec<u8> {
    let mut moov = mvhd();
    moov.extend(cat(traks));
    if !trexes.is_empty() {
        moov.extend(bx(b"mvex", &cat(trexes)));
    }
    cat(&[ftyp(), bx(b"moov", &moov)])
}

fn mfhd() -> Vec<u8> {
    full(b"mfhd", 0, 0, &1u32.to_be_bytes())
}

fn tfhd(track: u32, flags: u32, extra: &[u8]) -> Vec<u8> {
    let mut p = track.to_be_bytes().to_vec();
    p.extend_from_slice(extra);
    full(b"tfhd", 0, flags, &p)
}

fn trun(version: u8, flags: u32, count: u32, rest: &[u8]) -> Vec<u8> {
    let mut p = count.to_be_bytes().to_vec();
    p.extend_from_slice(rest);
    full(b"trun", version, flags, &p)
}

fn traf(parts: &[Vec<u8>]) -> Vec<u8> {
    bx(b"traf", &cat(parts))
}

fn moof(trafs: &[Vec<u8>]) -> Vec<u8> {
    let mut p = mfhd();
    p.extend(cat(trafs));
    bx(b"moof", &p)
}

const DEFAULT_BASE_IS_MOOF: u32 = 0x02_0000;
const TRUN_DATA_OFFSET: u32 = 0x001;
const TRUN_DURATION: u32 = 0x100;
const TRUN_SIZE: u32 = 0x200;

/// Appends a `moof` built by `build(data_offset)` followed by an `mdat`; the
/// closure receives the offset (relative to the `moof` box start) at which the
/// `mdat` payload begins. Returns the file, the `moof` start and the absolute
/// offset of the `mdat` payload.
fn with_moof(
    init: &[u8],
    build: impl Fn(i32) -> Vec<u8>,
    mdat_len: usize,
) -> (Vec<u8>, usize, usize) {
    let probe = build(0);
    let data_offset = (probe.len() + 8) as i32;
    let moof = build(data_offset);
    assert_eq!(moof.len(), probe.len());
    let mut file = init.to_vec();
    let moof_start = file.len();
    file.extend(moof);
    let payload = file.len() + 8;
    file.extend(bx(b"mdat", &vec![0xAB; mdat_len]));
    (file, moof_start, payload)
}

fn sizes_durs(sizes_and_durs: &[(u32, u32)]) -> Vec<u8> {
    let mut v = Vec::new();
    for (d, s) in sizes_and_durs {
        v.extend_from_slice(&d.to_be_bytes());
        v.extend_from_slice(&s.to_be_bytes());
    }
    v
}

#[test]
fn offsets_are_relative_to_the_moof_box_not_its_payload() {
    let init = init(&[trak(1, &[])], &[]);
    let (file, moof_start, payload) = with_moof(
        &init,
        |off| {
            let mut rest = off.to_be_bytes().to_vec();
            rest.extend(sizes_durs(&[(10, 50), (10, 60)]));
            moof(&[traf(&[
                tfhd(1, DEFAULT_BASE_IS_MOOF, &[]),
                trun(0, TRUN_DATA_OFFSET | TRUN_DURATION | TRUN_SIZE, 2, &rest),
            ])])
        },
        110,
    );
    assert!(moof_start > 0);
    let mp4 = demux_fragmented(&file).unwrap();
    let samples: Vec<_> = mp4.tracks[0].samples().map(|s| s.unwrap()).collect();
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].offset, payload as u64);
    assert_eq!(samples[0].size, 50);
    assert_eq!(samples[1].offset, payload as u64 + 50);
    assert_eq!(samples[1].size, 60);
    assert_eq!(samples[1].decoding, 10);
}

#[test]
fn largesize_moof_header_is_counted_in_the_base() {
    let init = init(&[trak(1, &[])], &[]);
    // Build the moof body, then wrap it with the 16-byte largesize header.
    let body = |off: i32| {
        let mut rest = off.to_be_bytes().to_vec();
        rest.extend(sizes_durs(&[(10, 40)]));
        let inner = cat(&[
            mfhd(),
            traf(&[
                tfhd(1, DEFAULT_BASE_IS_MOOF, &[]),
                trun(0, TRUN_DATA_OFFSET | TRUN_DURATION | TRUN_SIZE, 1, &rest),
            ]),
        ]);
        let mut b = 1u32.to_be_bytes().to_vec();
        b.extend_from_slice(b"moof");
        b.extend_from_slice(&((16 + inner.len()) as u64).to_be_bytes());
        b.extend(inner);
        b
    };
    let (file, _, payload) = with_moof(&init, body, 40);
    let mp4 = demux_fragmented(&file).unwrap();
    let s = mp4.tracks[0].samples().next().unwrap().unwrap();
    assert_eq!(s.offset, payload as u64);
}

#[test]
fn non_sync_is_bit_16_of_the_sample_flags() {
    let init = init(&[trak(1, &[])], &[]);
    // sync (depends_on=2), non-sync with depends_on, non-sync bit alone,
    // depends_on=1 without the non-sync bit (still a sync sample).
    let flags = [0x0200_0000u32, 0x0101_0000, 0x0001_0000, 0x0100_0000];
    let (file, _, _) = with_moof(
        &init,
        |off| {
            let mut rest = off.to_be_bytes().to_vec();
            for f in flags {
                rest.extend_from_slice(&10u32.to_be_bytes());
                rest.extend_from_slice(&f.to_be_bytes());
            }
            moof(&[traf(&[
                tfhd(1, DEFAULT_BASE_IS_MOOF, &[]),
                // Each record is: size, flags.
                trun(0, TRUN_DATA_OFFSET | TRUN_SIZE | 0x400, 4, &rest),
            ])])
        },
        40,
    );
    let mp4 = demux_fragmented(&file).unwrap();
    let keys: Vec<bool> = mp4.tracks[0]
        .samples()
        .map(|s| s.unwrap().keyframe)
        .collect();
    assert_eq!(keys, [true, false, false, true]);
}

#[test]
fn first_sample_flags_apply_to_the_first_sample_only() {
    let init = init(&[trak(1, &[])], &[trex(1, 10, 10, 0)]);
    let (file, _, _) = with_moof(
        &init,
        |off| {
            let mut rest = off.to_be_bytes().to_vec();
            rest.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // first: non-sync
            moof(&[traf(&[
                tfhd(1, DEFAULT_BASE_IS_MOOF, &[]),
                trun(0, TRUN_DATA_OFFSET | 0x004, 3, &rest),
            ])])
        },
        30,
    );
    let mp4 = demux_fragmented(&file).unwrap();
    let keys: Vec<bool> = mp4.tracks[0]
        .samples()
        .map(|s| s.unwrap().keyframe)
        .collect();
    assert_eq!(keys, [false, true, true]);
}

#[test]
fn tfhd_defaults_override_trex_and_trex_fills_the_rest() {
    // Track 1: tfhd gives duration + size + flags (and an ignored description
    // index); track 2 relies on its trex entirely.
    let init = init(
        &[trak(1, &[]), trak(2, &[])],
        &[trex(1, 1, 1, 0), trex(2, 7, 9, 0x0001_0000)],
    );
    let mut extra = 1u32.to_be_bytes().to_vec(); // sample_description_index
    extra.extend_from_slice(&25u32.to_be_bytes()); // duration
    extra.extend_from_slice(&12u32.to_be_bytes()); // size
    extra.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // flags: non-sync
    let (file, _, payload) = with_moof(
        &init,
        |off| {
            let first = off.to_be_bytes().to_vec();
            moof(&[
                traf(&[
                    tfhd(1, DEFAULT_BASE_IS_MOOF | 0x2 | 0x8 | 0x10 | 0x20, &extra),
                    trun(0, TRUN_DATA_OFFSET, 2, &first),
                ]),
                traf(&[
                    tfhd(2, DEFAULT_BASE_IS_MOOF, &[]),
                    trun(0, TRUN_DATA_OFFSET, 1, &(off + 24).to_be_bytes()),
                ]),
            ])
        },
        40,
    );
    let mp4 = demux_fragmented(&file).unwrap();
    let t1: Vec<_> = mp4.tracks[0].samples().map(|s| s.unwrap()).collect();
    assert_eq!(
        t1.iter()
            .map(|s| (s.size, s.duration, s.keyframe))
            .collect::<Vec<_>>(),
        [(12, 25, false), (12, 25, false)]
    );
    assert_eq!(t1[0].offset, payload as u64);
    assert_eq!(t1[1].offset, payload as u64 + 12);
    let t2: Vec<_> = mp4.tracks[1].samples().map(|s| s.unwrap()).collect();
    assert_eq!(t2.len(), 1);
    assert_eq!((t2[0].size, t2[0].duration, t2[0].keyframe), (9, 7, false));
    assert_eq!(t2[0].offset, payload as u64 + 24);
}

#[test]
fn without_a_base_the_next_traf_continues_where_the_previous_ended() {
    // tfhd flags 0 (no base_data_offset, no default-base-is-moof): the first
    // traf starts at the moof box, the second at the end of the first one's data.
    let init = init(&[trak(1, &[]), trak(2, &[])], &[]);
    let (file, _, payload) = with_moof(
        &init,
        |off| {
            let mut first = off.to_be_bytes().to_vec();
            first.extend(sizes_durs(&[(5, 50), (5, 60)]));
            moof(&[
                traf(&[
                    tfhd(1, 0, &[]),
                    trun(0, TRUN_DATA_OFFSET | TRUN_DURATION | TRUN_SIZE, 2, &first),
                ]),
                traf(&[
                    tfhd(2, 0, &[]),
                    trun(0, TRUN_DURATION | TRUN_SIZE, 1, &sizes_durs(&[(5, 30)])),
                ]),
            ])
        },
        140,
    );
    let mp4 = demux_fragmented(&file).unwrap();
    let t2 = mp4.tracks[1].samples().next().unwrap().unwrap();
    assert_eq!(t2.offset, payload as u64 + 110);
    assert_eq!(t2.size, 30);
}

#[test]
fn consecutive_truns_continue_and_moofs_append_in_order() {
    let init = init(&[trak(1, &[])], &[]);
    let build = |off: i32| {
        let mut first = off.to_be_bytes().to_vec();
        first.extend(sizes_durs(&[(1, 10)]));
        moof(&[traf(&[
            tfhd(1, DEFAULT_BASE_IS_MOOF, &[]),
            trun(0, TRUN_DATA_OFFSET | TRUN_DURATION | TRUN_SIZE, 1, &first),
            trun(0, TRUN_DURATION | TRUN_SIZE, 1, &sizes_durs(&[(2, 20)])),
        ])])
    };
    let (mut file, _, payload) = with_moof(&init, build, 30);
    // A second fragment for the same track, with its own mdat.
    let second_start = file.len();
    let probe = build(0);
    file.extend(build((probe.len() + 8) as i32));
    let second_payload = file.len() + 8;
    file.extend(bx(b"mdat", &[0xCD; 30]));
    assert!(second_start > payload);

    let mp4 = demux_fragmented(&file).unwrap();
    let s: Vec<_> = mp4.tracks[0].samples().map(|s| s.unwrap()).collect();
    let offs: Vec<u64> = s.iter().map(|s| s.offset).collect();
    assert_eq!(
        offs,
        [
            payload as u64,
            payload as u64 + 10,
            second_payload as u64,
            second_payload as u64 + 10
        ]
    );
    // Durations 1,2,1,2 -> four runs; decoding times accumulate.
    assert_eq!(
        s.iter().map(|s| s.decoding).collect::<Vec<_>>(),
        [0, 1, 3, 4]
    );
}

#[test]
fn composition_offsets_follow_trun_version() {
    let init = init(&[trak(1, &[])], &[trex(1, 10, 5, 0)]);
    let (file, _, _) = with_moof(
        &init,
        |off| {
            let mut rest = off.to_be_bytes().to_vec();
            for c in [20i32, -10, 0] {
                rest.extend_from_slice(&c.to_be_bytes());
            }
            moof(&[traf(&[
                tfhd(1, DEFAULT_BASE_IS_MOOF, &[]),
                trun(1, TRUN_DATA_OFFSET | 0x800, 3, &rest),
            ])])
        },
        15,
    );
    let mp4 = demux_fragmented(&file).unwrap();
    let pts: Vec<i64> = mp4.tracks[0]
        .samples()
        .map(|s| s.unwrap().presentation)
        .collect();
    assert_eq!(pts, [20, 0, 20]);
}

#[test]
fn moof_without_tfhd_or_for_unknown_track_changes_nothing() {
    let init = init(&[trak(1, &[])], &[]);
    let moof = moof(&[
        traf(&[trun(0, TRUN_SIZE, 1, &20u32.to_be_bytes())]),
        traf(&[
            tfhd(99, DEFAULT_BASE_IS_MOOF, &[]),
            trun(0, TRUN_SIZE, 1, &20u32.to_be_bytes()),
        ]),
    ]);
    let file = cat(&[init, moof, bx(b"mdat", &[0; 64])]);
    let mp4 = demux_fragmented(&file).unwrap();
    assert_eq!(mp4.tracks[0].table.sample_count, 0);
    assert!(mp4.tracks[0].samples().next().is_none());
}

#[test]
fn sample_ranges_must_stay_inside_the_file() {
    let init = init(&[trak(1, &[])], &[]);
    // 100-byte sample but only 40 bytes of mdat follow.
    let (file, _, _) = with_moof(
        &init,
        |off| {
            let mut rest = off.to_be_bytes().to_vec();
            rest.extend_from_slice(&100u32.to_be_bytes());
            moof(&[traf(&[
                tfhd(1, DEFAULT_BASE_IS_MOOF, &[]),
                trun(0, TRUN_DATA_OFFSET | TRUN_SIZE, 1, &rest),
            ])])
        },
        40,
    );
    assert!(matches!(
        demux_fragmented(&file),
        Err(Error::Truncated {
            what: "sample payload",
            ..
        })
    ));
}

#[test]
fn offset_arithmetic_never_wraps_or_panics() {
    let init = init(&[trak(1, &[])], &[]);
    // data_offset pointing before byte 0.
    let neg = cat(&[
        init.clone(),
        moof(&[traf(&[
            tfhd(1, DEFAULT_BASE_IS_MOOF, &[]),
            trun(0, TRUN_DATA_OFFSET | TRUN_SIZE, 1, &{
                let mut r = i32::MIN.to_be_bytes().to_vec();
                r.extend_from_slice(&4u32.to_be_bytes());
                r
            }),
        ])]),
        bx(b"mdat", &[0; 16]),
    ]);
    assert!(matches!(
        demux_fragmented(&neg),
        Err(Error::BadValue("trun data_offset leaves the file"))
    ));

    // base_data_offset at the top of u64: adding a sample size overflows.
    let big = cat(&[
        init,
        moof(&[traf(&[
            tfhd(1, 0x1, &(u64::MAX - 2).to_be_bytes()),
            trun(0, TRUN_SIZE, 1, &8u32.to_be_bytes()),
        ])]),
        bx(b"mdat", &[0; 16]),
    ]);
    assert!(matches!(
        demux_fragmented(&big),
        Err(Error::Truncated {
            what: "sample payload",
            ..
        })
    ));
}

#[test]
fn huge_sample_counts_are_refused_before_allocating() {
    let init = init(&[trak(1, &[])], &[]);
    // No per-sample fields and a zero default size: 4 billion zero-length
    // samples would otherwise be materialized one by one.
    let file = cat(&[
        init.clone(),
        moof(&[traf(&[
            tfhd(1, DEFAULT_BASE_IS_MOOF, &[]),
            trun(0, 0, u32::MAX, &[]),
        ])]),
    ]);
    assert!(matches!(
        demux_fragmented(&file),
        Err(Error::BadValue("trun sample_count exceeds the file size"))
    ));

    // The bound is cumulative: several truns cannot add up past the file size.
    let build = |c1: u32, c2: u32| {
        cat(&[
            init.clone(),
            moof(&[traf(&[
                tfhd(1, DEFAULT_BASE_IS_MOOF, &[]),
                trun(0, 0, c1, &[]),
                trun(0, 0, c2, &[]),
            ])]),
        ])
    };
    let len = build(0, 0).len() as u32;
    assert!(demux_fragmented(&build(len / 2, len / 2)).is_ok());
    assert!(matches!(
        demux_fragmented(&build(len / 2 + 1, len / 2 + 1)),
        Err(Error::BadValue("trun sample_count exceeds the file size"))
    ));
}

#[test]
fn fragments_after_samples_in_moov_are_refused_not_dropped() {
    let stts = full(b"stts", 0, 0, &[0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 10]);
    let stsc = full(
        b"stsc",
        0,
        0,
        &[0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1],
    );
    let stsz = full(b"stsz", 0, 0, &[0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 4]);
    let stco = full(b"stco", 0, 0, &[0, 0, 0, 1, 0, 0, 0, 0]);
    let init = init(&[trak(1, &[stts, stsc, stsz, stco])], &[]);
    let file = cat(&[
        init,
        moof(&[traf(&[
            tfhd(1, DEFAULT_BASE_IS_MOOF | 0x10, &4u32.to_be_bytes()),
            trun(0, 0, 1, &[]),
        ])]),
        bx(b"mdat", &[0; 16]),
    ]);
    assert!(matches!(
        demux_fragmented(&file),
        Err(Error::Unsupported("fragments after samples in moov"))
    ));
}

#[test]
fn container_level_refusals() {
    assert!(matches!(
        demux_fragmented(&[]),
        Err(Error::InvalidMagic { what: "empty file" })
    ));
    assert!(matches!(
        demux_fragmented(&bx(b"free", &[0; 8])),
        Err(Error::InvalidMagic {
            what: "first box is not ftyp"
        })
    ));
    assert!(matches!(
        demux_fragmented(&ftyp()),
        Err(Error::BadValue("no moov box"))
    ));
    let one = init(&[trak(1, &[])], &[]);
    let moov_only = one[ftyp().len()..].to_vec();
    let two = cat(&[one, moov_only]);
    assert!(matches!(
        demux_fragmented(&two),
        Err(Error::BadValue("more than one moov box"))
    ));
    // A truncated tfhd inside an otherwise valid moof surfaces as Truncated.
    let bad = cat(&[
        init(&[trak(1, &[])], &[]),
        moof(&[traf(&[full(b"tfhd", 0, 0, &[0, 0])])]),
    ]);
    assert!(matches!(
        demux_fragmented(&bad),
        Err(Error::Truncated { .. })
    ));
}

#[test]
fn parse_mvex_reads_trex_and_skips_other_boxes() {
    let mvex = cat(&[bx(b"mehd", &[0; 8]), trex(7, 3, 4, 5), trex(8, 6, 7, 8)]);
    let d = parse_mvex(&mvex).unwrap();
    assert_eq!(d.len(), 2);
    assert_eq!(
        (
            d[0].track_id,
            d[0].default_sample_description_index,
            d[0].default_sample_duration,
            d[0].default_sample_size,
            d[0].default_sample_flags
        ),
        (7, 1, 3, 4, 5)
    );
    assert_eq!(d[1].track_id, 8);
    assert!(parse_mvex(&[]).unwrap().is_empty());
    // trex cut short is an error, not a default-filled entry.
    assert!(parse_mvex(&full(b"trex", 0, 0, &[0, 0, 0, 1])).is_err());
}

#[test]
fn run_length_tables_merge_equal_neighbours() {
    let init = init(&[trak(1, &[])], &[]);
    let (file, _, _) = with_moof(
        &init,
        |off| {
            let mut rest = off.to_be_bytes().to_vec();
            rest.extend(sizes_durs(&[(10, 1), (10, 1), (20, 1), (20, 1), (10, 1)]));
            moof(&[traf(&[
                tfhd(1, DEFAULT_BASE_IS_MOOF, &[]),
                trun(0, TRUN_DATA_OFFSET | TRUN_DURATION | TRUN_SIZE, 5, &rest),
            ])])
        },
        5,
    );
    let mp4 = demux_fragmented(&file).unwrap();
    let t = &mp4.tracks[0].table;
    assert_eq!(
        t.times
            .iter()
            .map(|r| (r.count, r.delta))
            .collect::<Vec<_>>(),
        [(2, 10), (2, 20), (1, 10)]
    );
    // All samples sync and no composition offsets: neither table is kept.
    assert!(t.sync.is_none());
    assert!(t.comp.is_empty());
}
