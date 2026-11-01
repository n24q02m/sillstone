//! The C ABI record walk of `sillstone-mp4`, checked against bytes this
//! file writes by hand from the wire format documented at the top of
//! `ffi.rs`. The in-crate tests pin a committed fixture's digest; this
//! suite covers what that fixture does not: audio and unrecognised
//! sample entries, a present language code, several tracks, and every
//! null-pointer refusal.

use sillstone_mp4::ffi::{
    PITH_E_INVALID, PITH_E_REJECTED, PITH_ERR_TRUNCATED, PITH_OK, sillstone_mp4_demux,
    sillstone_mp4_free,
};

fn bx(four: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(payload.len() + 8);
    v.extend_from_slice(&(payload.len() as u32 + 8).to_be_bytes());
    v.extend_from_slice(four);
    v.extend_from_slice(payload);
    v
}

fn full(four: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut p = vec![0, 0, 0, 0];
    p.extend_from_slice(payload);
    bx(four, &p)
}

fn be32(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

fn mvhd() -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&be32(0));
    p.extend_from_slice(&be32(0));
    p.extend_from_slice(&be32(1000)); // timescale
    p.extend_from_slice(&be32(2000)); // duration
    p.extend_from_slice(&[0; 80]); // rate .. next_track_id
    full(b"mvhd", &p)
}

fn tkhd(id: u32, w: u16, h: u16) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&be32(0));
    p.extend_from_slice(&be32(0));
    p.extend_from_slice(&be32(id));
    p.extend_from_slice(&be32(0));
    p.extend_from_slice(&be32(2000));
    p.extend_from_slice(&[0; 8 + 2 + 2 + 2 + 2 + 36]);
    p.extend_from_slice(&be32(u32::from(w) << 16));
    p.extend_from_slice(&be32(u32::from(h) << 16));
    full(b"tkhd", &p)
}

fn mdhd(timescale: u32, duration: u32, packed_lang: u16) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&be32(0));
    p.extend_from_slice(&be32(0));
    p.extend_from_slice(&be32(timescale));
    p.extend_from_slice(&be32(duration));
    p.extend_from_slice(&packed_lang.to_be_bytes());
    p.extend_from_slice(&[0, 0]);
    full(b"mdhd", &p)
}

fn hdlr(kind: &[u8; 4]) -> Vec<u8> {
    let mut p = be32(0).to_vec();
    p.extend_from_slice(kind);
    p.extend_from_slice(&[0; 12]);
    p.extend_from_slice(b"h\0");
    full(b"hdlr", &p)
}

/// One sample entry: `size, coding, 6 reserved, data-ref 1, body`.
fn entry(coding: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut p = vec![0; 6];
    p.extend_from_slice(&1u16.to_be_bytes());
    p.extend_from_slice(body);
    bx(coding, &p)
}

fn visual_entry(w: u16, h: u16, children: &[u8]) -> Vec<u8> {
    let mut body = vec![0; 16 + 2 + 2 + 4 + 4 + 4 + 2 + 32 + 2 + 2];
    body[16..18].copy_from_slice(&w.to_be_bytes());
    body[18..20].copy_from_slice(&h.to_be_bytes());
    body.extend_from_slice(children);
    entry(b"avc1", &body)
}

fn audio_entry(coding: &[u8; 4], channels: u16, rate: u32, children: &[u8]) -> Vec<u8> {
    // After the 8 shared bytes: 8 reserved, channels, samplesize,
    // pre-defined, reserved, samplerate (16.16).
    let mut body = vec![0; 8];
    body.extend_from_slice(&channels.to_be_bytes());
    body.extend_from_slice(&16u16.to_be_bytes());
    body.extend_from_slice(&[0; 4]);
    body.extend_from_slice(&(rate << 16).to_be_bytes());
    body.extend_from_slice(children);
    entry(coding, &body)
}

/// A one-sample track: `size` bytes at `offset`, duration 10 ticks.
#[allow(clippy::too_many_arguments)]
fn trak(
    id: u32,
    handler: &[u8; 4],
    lang: u16,
    stsd_entry: &[u8],
    offset: u32,
    size: u32,
    w: u16,
    h: u16,
) -> Vec<u8> {
    let mut stsd = be32(1).to_vec();
    stsd.extend_from_slice(stsd_entry);
    let mut stbl = full(b"stsd", &stsd);
    stbl.extend_from_slice(&full(
        b"stts",
        &[&be32(1)[..], &be32(1), &be32(10)].concat(),
    ));
    stbl.extend_from_slice(&full(
        b"stsc",
        &[&be32(1)[..], &be32(1), &be32(1), &be32(1)].concat(),
    ));
    stbl.extend_from_slice(&full(
        b"stsz",
        &[&be32(0)[..], &be32(1), &be32(size)].concat(),
    ));
    stbl.extend_from_slice(&full(b"stco", &[&be32(1)[..], &be32(offset)].concat()));
    let minf = bx(b"minf", &bx(b"stbl", &stbl));
    let mut mdia = mdhd(48_000, 480, lang);
    mdia.extend_from_slice(&hdlr(handler));
    mdia.extend_from_slice(&minf);
    let mut trak = tkhd(id, w, h);
    trak.extend_from_slice(&bx(b"mdia", &mdia));
    bx(b"trak", &trak)
}

const PAYLOAD: [u8; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
const FRA: u16 = (6 << 10) | (18 << 5) | 1; // "fra"

fn ftyp() -> Vec<u8> {
    let mut p = b"M4A ".to_vec();
    p.extend_from_slice(&be32(0));
    p.extend_from_slice(b"M4A ");
    p.extend_from_slice(b"isom");
    bx(b"ftyp", &p)
}

fn three_track_file() -> Vec<u8> {
    let build = |base: u32| {
        let avcc = bx(b"avcC", &[1, 0x64, 0, 0x1F]);
        let video = visual_entry(320, 240, &[avcc, bx(b"esds", &[0xAA])].concat());
        let audio = audio_entry(b"mp4a", 2, 44_100, &bx(b"esds", &[7, 7, 7]));
        // `tx3g` is neither visual nor audio: surfaced as `Other`.
        let other = entry(b"tx3g", &[0; 4]);
        let mut moov = mvhd();
        moov.extend_from_slice(&trak(1, b"vide", FRA, &video, base, 4, 320, 240));
        moov.extend_from_slice(&trak(2, b"soun", FRA, &audio, base + 4, 4, 0, 0));
        // A zero language field is not three letters: no language.
        moov.extend_from_slice(&trak(3, b"text", 0, &other, base + 8, 4, 0, 0));
        bx(b"moov", &moov)
    };
    let ftyp = ftyp();
    let moov_len = build(0).len();
    let mdat_payload = (ftyp.len() + moov_len + 8) as u32;
    let mut file = ftyp;
    file.extend_from_slice(&build(mdat_payload));
    file.extend_from_slice(&bx(b"mdat", &PAYLOAD));
    file
}

struct Wire(Vec<u8>);

impl Wire {
    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn u64(&mut self, v: u64) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn raw(&mut self, v: &[u8]) -> &mut Self {
        self.0.extend_from_slice(v);
        self
    }
    fn blob(&mut self, v: &[u8]) -> &mut Self {
        self.u32(v.len() as u32).raw(v)
    }
}

/// Runs the raw FFI, copies the buffer out and releases it.
fn walk(input: &[u8]) -> (i32, i32, Vec<u8>) {
    let mut out: *mut u8 = core::ptr::null_mut();
    let mut out_len = 0usize;
    let mut err = -99i32;
    let status = unsafe {
        sillstone_mp4_demux(
            input.as_ptr(),
            input.len(),
            &mut out,
            &mut out_len,
            &mut err,
        )
    };
    let bytes = if out.is_null() {
        assert_eq!(out_len, 0, "a refusal reports a zero length");
        Vec::new()
    } else {
        let copy = unsafe { core::slice::from_raw_parts(out, out_len) }.to_vec();
        unsafe { sillstone_mp4_free(out, out_len) };
        copy
    };
    (status, err, bytes)
}

#[test]
fn record_walk_covers_visual_audio_and_other_entries() {
    let file = three_track_file();
    let mdat = (file.len() - PAYLOAD.len()) as u64;
    let (status, err, got) = walk(&file);
    assert_eq!((status, err), (PITH_OK, PITH_OK));

    let mut w = Wire(Vec::new());
    w.raw(b"M4A ").u32(2).raw(b"M4A ").raw(b"isom");
    w.u32(1000).u64(2000).u32(3);
    let sample = |w: &mut Wire, at: u64| {
        w.u32(1).u64(at).u32(4).u64(0).u64(0).u32(10).u8(1);
    };
    // Track 1: video with a language, one visual entry carrying avcC + esds.
    w.u32(1).u32(48_000).u64(480).u8(1).raw(b"fra");
    w.raw(b"vide").u32(320).u32(240);
    sample(&mut w, mdat);
    w.u32(1).u8(0).raw(b"avc1").u32(320).u32(240);
    w.blob(&[1, 0x64, 0, 0x1F]).blob(&[0xAA]);
    // Track 2: audio, one audio entry (the rate is the integer part of 16.16).
    w.u32(2).u32(48_000).u64(480).u8(1).raw(b"fra");
    w.raw(b"soun").u32(0).u32(0);
    sample(&mut w, mdat + 4);
    w.u32(1).u8(1).raw(b"mp4a").u32(2).u32(44_100);
    w.blob(&[7, 7, 7]);
    // Track 3: no language (all four bytes zero), an `Other` entry.
    w.u32(3).u32(48_000).u64(480).raw(&[0, 0, 0, 0]);
    w.raw(b"text").u32(0).u32(0);
    sample(&mut w, mdat + 8);
    w.u32(1).u8(2).raw(b"tx3g");
    assert_eq!(got, w.0);
}

#[test]
fn audio_entry_without_esds_serializes_an_empty_blob() {
    let audio = audio_entry(b"Opus", 6, 48_000, &[]);
    let mut moov = mvhd();
    moov.extend_from_slice(&trak(1, b"soun", FRA, &audio, 0, 0, 0, 0));
    let mut file = ftyp();
    file.extend_from_slice(&bx(b"moov", &moov));
    let (status, err, got) = walk(&file);
    assert_eq!((status, err), (PITH_OK, PITH_OK));
    // The last 4 + 1 + 4 + 4 + 4 + 4 bytes are the entry: kind, coding,
    // channels, rate, then a zero-length esds blob.
    let tail = &got[got.len() - (1 + 4 + 4 + 4 + 4)..];
    let mut w = Wire(Vec::new());
    w.u8(1).raw(b"Opus").u32(6).u32(48_000).blob(&[]);
    assert_eq!(tail, w.0.as_slice());
}

#[test]
fn null_arguments_are_invalid_and_leave_the_outputs_alone() {
    let file = three_track_file();
    let mut out: *mut u8 = core::ptr::null_mut();
    let mut out_len = 7usize;
    let mut err = -99i32;
    let ok = file.as_ptr();
    unsafe {
        assert_eq!(
            sillstone_mp4_demux(
                ok,
                file.len(),
                core::ptr::null_mut(),
                &mut out_len,
                &mut err
            ),
            PITH_E_INVALID
        );
        assert_eq!(
            sillstone_mp4_demux(ok, file.len(), &mut out, core::ptr::null_mut(), &mut err),
            PITH_E_INVALID
        );
        assert_eq!(
            sillstone_mp4_demux(
                ok,
                file.len(),
                &mut out,
                &mut out_len,
                core::ptr::null_mut()
            ),
            PITH_E_INVALID
        );
        assert_eq!(
            sillstone_mp4_demux(core::ptr::null(), 0, &mut out, &mut out_len, &mut err),
            PITH_E_INVALID
        );
        // Freeing null is a documented no-op.
        sillstone_mp4_free(core::ptr::null_mut(), 0);
    }
    assert!(out.is_null());
    assert_eq!((out_len, err), (7, -99));
}

#[test]
fn a_cut_file_is_rejected_with_a_kind_code_and_no_buffer() {
    let file = three_track_file();
    // Cut inside the last trak: the moov box claims more than is there.
    let (status, err, got) = walk(&file[..file.len() - PAYLOAD.len() - 30]);
    assert_eq!(status, PITH_E_REJECTED);
    assert_eq!(err, PITH_ERR_TRUNCATED);
    assert!(got.is_empty());
}
