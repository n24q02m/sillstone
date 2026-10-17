//! Test for fragmented MP4 demuxing.

use sillstone_mp4::fragmented::demux_fragmented;

fn box_bytes(four: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = (8 + payload.len()) as u32;
    let mut b = Vec::with_capacity(size as usize);
    b.extend_from_slice(&size.to_be_bytes());
    b.extend_from_slice(four);
    b.extend_from_slice(payload);
    b
}

fn full_box_bytes(four: &[u8; 4], version: u8, flags: u32, payload: &[u8]) -> Vec<u8> {
    let size = (12 + payload.len()) as u32;
    let mut b = Vec::with_capacity(size as usize);
    b.extend_from_slice(&size.to_be_bytes());
    b.extend_from_slice(four);
    b.push(version);
    b.extend_from_slice(&flags.to_be_bytes()[1..4]);
    b.extend_from_slice(payload);
    b
}

#[test]
fn test_fragmented_demux() {
    let mut file = Vec::new();

    // ftyp
    let mut ftyp_payload = Vec::new();
    ftyp_payload.extend_from_slice(b"isom");
    ftyp_payload.extend_from_slice(&0x200u32.to_be_bytes());
    ftyp_payload.extend_from_slice(b"isom");
    file.extend_from_slice(&box_bytes(b"ftyp", &ftyp_payload));

    // moov
    let mut moov_payload = Vec::new();
    // mvhd
    let mut mvhd_payload = vec![0u8; 96];
    mvhd_payload[12..16].copy_from_slice(&1000u32.to_be_bytes()); // timescale 1000
    mvhd_payload[16..20].copy_from_slice(&4000u32.to_be_bytes()); // duration 4000
    moov_payload.extend_from_slice(&full_box_bytes(b"mvhd", 0, 0, &mvhd_payload));

    // trak
    let mut trak_payload = Vec::new();
    // tkhd
    let mut tkhd_payload = vec![0u8; 80];
    tkhd_payload[8..12].copy_from_slice(&1u32.to_be_bytes()); // track_id = 1
    tkhd_payload[72..76].copy_from_slice(&(1920u32 << 16).to_be_bytes());
    tkhd_payload[76..80].copy_from_slice(&(1080u32 << 16).to_be_bytes());
    trak_payload.extend_from_slice(&full_box_bytes(b"tkhd", 0, 0, &tkhd_payload));

    // mdia
    let mut mdia_payload = Vec::new();
    let mut mdhd_payload = vec![0u8; 20];
    mdhd_payload[8..12].copy_from_slice(&1000u32.to_be_bytes()); // timescale 1000
    mdhd_payload[12..16].copy_from_slice(&4000u32.to_be_bytes());
    mdia_payload.extend_from_slice(&full_box_bytes(b"mdhd", 0, 0, &mdhd_payload));

    // hdlr
    let mut hdlr_payload = vec![0u8; 20];
    hdlr_payload[8..12].copy_from_slice(b"vide");
    mdia_payload.extend_from_slice(&full_box_bytes(b"hdlr", 0, 0, &hdlr_payload));

    // minf
    let mut minf_payload = Vec::new();
    // stbl
    let mut stbl_payload = Vec::new();
    // stsd
    let mut stsd_payload = vec![0u8; 8];
    stsd_payload[4..8].copy_from_slice(&1u32.to_be_bytes()); // 1 entry
    let mut entry_payload = vec![0u8; 70];
    entry_payload[24..26].copy_from_slice(&1920u16.to_be_bytes());
    entry_payload[26..28].copy_from_slice(&1080u16.to_be_bytes());
    stsd_payload.extend_from_slice(&box_bytes(b"avc1", &entry_payload));
    stbl_payload.extend_from_slice(&full_box_bytes(b"stsd", 0, 0, &stsd_payload));

    // empty stts, stsc, stsz, stco
    stbl_payload.extend_from_slice(&full_box_bytes(b"stts", 0, 0, &[0; 4]));
    stbl_payload.extend_from_slice(&full_box_bytes(b"stsc", 0, 0, &[0; 4]));
    stbl_payload.extend_from_slice(&full_box_bytes(b"stsz", 0, 0, &[0; 8]));
    stbl_payload.extend_from_slice(&full_box_bytes(b"stco", 0, 0, &[0; 4]));

    minf_payload.extend_from_slice(&box_bytes(b"stbl", &stbl_payload));
    mdia_payload.extend_from_slice(&box_bytes(b"minf", &minf_payload));
    trak_payload.extend_from_slice(&box_bytes(b"mdia", &mdia_payload));
    moov_payload.extend_from_slice(&box_bytes(b"trak", &trak_payload));

    // mvex
    let mut mvex_payload = Vec::new();
    let mut trex_payload = vec![0u8; 20];
    trex_payload[0..4].copy_from_slice(&1u32.to_be_bytes()); // track_id = 1
    trex_payload[4..8].copy_from_slice(&1u32.to_be_bytes()); // def desc index = 1
    trex_payload[8..12].copy_from_slice(&1000u32.to_be_bytes()); // def dur = 1000
    trex_payload[12..16].copy_from_slice(&100u32.to_be_bytes()); // def size = 100
    trex_payload[16..20].copy_from_slice(&0u32.to_be_bytes()); // def flags = 0
    mvex_payload.extend_from_slice(&full_box_bytes(b"trex", 0, 0, &trex_payload));
    moov_payload.extend_from_slice(&box_bytes(b"mvex", &mvex_payload));

    file.extend_from_slice(&box_bytes(b"moov", &moov_payload));

    // moof
    let mut moof_payload = Vec::new();
    // mfhd
    let mut mfhd_payload = vec![0u8; 4];
    mfhd_payload[0..4].copy_from_slice(&1u32.to_be_bytes()); // seq = 1
    moof_payload.extend_from_slice(&full_box_bytes(b"mfhd", 0, 0, &mfhd_payload));

    // traf
    let mut traf_payload = Vec::new();
    // tfhd
    let mut tfhd_payload = vec![0u8; 4];
    tfhd_payload[0..4].copy_from_slice(&1u32.to_be_bytes()); // track_id = 1
    traf_payload.extend_from_slice(&full_box_bytes(b"tfhd", 0, 0x020000, &tfhd_payload)); // default-base-is-moof

    // trun
    let mut trun_payload = Vec::new();
    trun_payload.extend_from_slice(&2u32.to_be_bytes()); // 2 samples
    trun_payload.extend_from_slice(&1000i32.to_be_bytes()); // data_offset = 1000
    // sample 1: dur=1000, size=50
    trun_payload.extend_from_slice(&1000u32.to_be_bytes());
    trun_payload.extend_from_slice(&50u32.to_be_bytes());
    // sample 2: dur=1000, size=60
    trun_payload.extend_from_slice(&1000u32.to_be_bytes());
    trun_payload.extend_from_slice(&60u32.to_be_bytes());
    // trun flags: data_offset_present (0x01) | duration_present (0x100) | size_present (0x200) = 0x301
    traf_payload.extend_from_slice(&full_box_bytes(b"trun", 0, 0x000301, &trun_payload));

    moof_payload.extend_from_slice(&box_bytes(b"traf", &traf_payload));
    file.extend_from_slice(&box_bytes(b"moof", &moof_payload));

    let demuxed = demux_fragmented(&file).expect("demux fragmented succeeds");
    assert_eq!(demuxed.tracks.len(), 1);
    let track = &demuxed.tracks[0];
    assert_eq!(track.table.sample_count, 2);
    assert_eq!(track.table.sizes, vec![50, 60]);
    assert_eq!(track.table.times.len(), 1);
    assert_eq!(track.table.times[0].count, 2);
    assert_eq!(track.table.times[0].delta, 1000);
}
