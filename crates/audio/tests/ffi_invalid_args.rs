//! Refusals and the PCM success path of the audio C ABI, called from a
//! consumer crate the way the language SDKs call it.
//!
//! Every refusal asserts the exact status code and that the output
//! slots stay untouched, so a failed call can never hand a half-built
//! buffer to the caller. Known answers (frame count, peak count and
//! fingerprint of the `voices-signature-mono` corpus) are the values
//! pinned in `reference.json`, not values recomputed from this crate.

use sillstone_audio::ffi::{
    SILLSTONE_E_INVALID, SILLSTONE_E_REJECTED, SILLSTONE_OK, sillstone_audio_decode_wav,
    sillstone_audio_free, sillstone_audio_index_add, sillstone_audio_index_free,
    sillstone_audio_index_new, sillstone_audio_match, sillstone_audio_signature_pcm,
    sillstone_audio_signature_wav, sillstone_audio_voices_pcm,
};

/// Output slots as the caller would pass them.
struct Slots {
    out: *mut u8,
    out_len: usize,
}

impl Slots {
    fn new() -> Self {
        Self {
            out: core::ptr::null_mut(),
            out_len: 0,
        }
    }

    /// The refusal contract: nothing was allocated and nothing was written.
    fn assert_untouched(&self, what: &str) {
        assert!(self.out.is_null(), "{what}: out pointer was written");
        assert_eq!(self.out_len, 0, "{what}: out_len was written");
    }

    /// Copies the handed-out buffer and frees it.
    fn take(&mut self) -> Vec<u8> {
        let copied = unsafe { core::slice::from_raw_parts(self.out, self.out_len) }.to_vec();
        unsafe { sillstone_audio_free(self.out, self.out_len) };
        self.out = core::ptr::null_mut();
        self.out_len = 0;
        copied
    }
}

fn be32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn be64(bytes: &[u8], at: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&bytes[at..at + 8]);
    u64::from_be_bytes(b)
}

/// The committed WAV fixture.
fn tone_wav() -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/tone.wav",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("fixture")
}

/// The `voices-signature-mono` corpus (seed and length of the pinned
/// vector), synthesized inside the library.
fn mono_corpus() -> Vec<u8> {
    let mut s = Slots::new();
    let status =
        unsafe { sillstone_audio_voices_pcm(0xC0_FFEE, 66_150, &mut s.out, &mut s.out_len) };
    assert_eq!(status, SILLSTONE_OK);
    assert_eq!(s.out_len, 66_150 * 4);
    s.take()
}

#[test]
fn wav_exports_refuse_a_null_data_pointer() {
    let wav = tone_wav();

    let mut s = Slots::new();
    let status =
        unsafe { sillstone_audio_decode_wav(core::ptr::null(), 0, &mut s.out, &mut s.out_len) };
    assert_eq!(status, SILLSTONE_E_INVALID, "decode_wav null data");
    s.assert_untouched("decode_wav null data");

    let status =
        unsafe { sillstone_audio_signature_wav(core::ptr::null(), 0, &mut s.out, &mut s.out_len) };
    assert_eq!(status, SILLSTONE_E_INVALID, "signature_wav null data");
    s.assert_untouched("signature_wav null data");

    // The valid call next to the refusals still succeeds.
    let status = unsafe {
        sillstone_audio_signature_wav(wav.as_ptr(), wav.len(), &mut s.out, &mut s.out_len)
    };
    assert_eq!(status, SILLSTONE_OK);
    let stream = s.take();
    assert_eq!(be32(&stream, 0), 20, "frames");
    assert_eq!(be32(&stream, 4), 42, "peak_count");
}

#[test]
fn wav_exports_refuse_a_null_out_slot() {
    let wav = tone_wav();
    let mut out_len: usize = 0;
    let status = unsafe {
        sillstone_audio_signature_wav(wav.as_ptr(), wav.len(), core::ptr::null_mut(), &mut out_len)
    };
    assert_eq!(status, SILLSTONE_E_INVALID);
    assert_eq!(out_len, 0);

    let mut out: *mut u8 = core::ptr::null_mut();
    let status = unsafe {
        sillstone_audio_signature_wav(wav.as_ptr(), wav.len(), &mut out, core::ptr::null_mut())
    };
    assert_eq!(status, SILLSTONE_E_INVALID);
    assert!(out.is_null());
}

#[test]
fn signature_wav_rejects_a_stream_that_is_not_a_wav() {
    let garbage = [0u8; 16];
    let mut s = Slots::new();
    let status = unsafe {
        sillstone_audio_signature_wav(garbage.as_ptr(), garbage.len(), &mut s.out, &mut s.out_len)
    };
    assert_eq!(status, SILLSTONE_E_REJECTED);
    s.assert_untouched("signature_wav garbage");
}

#[test]
fn signature_pcm_gives_the_pinned_vector_and_refuses_bad_arguments() {
    let pcm = mono_corpus();

    // reference.json `voices-signature-mono`: 31 frames, 53 peaks.
    let mut s = Slots::new();
    let status = unsafe {
        sillstone_audio_signature_pcm(pcm.as_ptr(), pcm.len(), 1, &mut s.out, &mut s.out_len)
    };
    assert_eq!(status, SILLSTONE_OK);
    let stream = s.take();
    assert_eq!(be32(&stream, 0), 31, "frames");
    assert_eq!(be32(&stream, 4), 53, "peak_count");
    assert_eq!(be64(&stream, 8), 0x0040_0000_0290_9467, "fingerprint");
    assert_eq!(stream.len(), 16 + 53 * 6);

    let cases: [(&str, *const u8, usize, u32); 6] = [
        ("null data", core::ptr::null(), 4, 1),
        ("partial sample", pcm.as_ptr(), 6, 1),
        ("zero channels", pcm.as_ptr(), 4, 0),
        (
            "channels beyond u16",
            pcm.as_ptr(),
            4,
            u32::from(u16::MAX) + 1,
        ),
        // 65537 wraps to 1 channel if the conversion truncates: it must be refused.
        (
            "channels wrapping to 1",
            pcm.as_ptr(),
            4,
            u32::from(u16::MAX) + 2,
        ),
        ("null data, zero length", core::ptr::null(), 0, 1),
    ];
    for (what, data, len, channels) in cases {
        let mut s = Slots::new();
        let status = unsafe {
            sillstone_audio_signature_pcm(data, len, channels, &mut s.out, &mut s.out_len)
        };
        assert_eq!(status, SILLSTONE_E_INVALID, "{what}");
        s.assert_untouched(what);
    }

    // One sample cannot be a whole number of 2-channel frames: the
    // signature stage refuses it, which is a rejection and not an
    // argument error.
    let mut s = Slots::new();
    let status =
        unsafe { sillstone_audio_signature_pcm(pcm.as_ptr(), 4, 2, &mut s.out, &mut s.out_len) };
    assert_eq!(status, SILLSTONE_E_REJECTED);
    s.assert_untouched("partial interleaved frame");
}

#[test]
fn signature_pcm_refuses_each_null_out_slot() {
    let pcm = [0u8; 4];
    let mut out_len: usize = 0;
    let status = unsafe {
        sillstone_audio_signature_pcm(
            pcm.as_ptr(),
            pcm.len(),
            1,
            core::ptr::null_mut(),
            &mut out_len,
        )
    };
    assert_eq!(status, SILLSTONE_E_INVALID, "null out");
    assert_eq!(out_len, 0);

    let mut out: *mut u8 = core::ptr::null_mut();
    let status = unsafe {
        sillstone_audio_signature_pcm(pcm.as_ptr(), pcm.len(), 1, &mut out, core::ptr::null_mut())
    };
    assert_eq!(status, SILLSTONE_E_INVALID, "null out_len");
    assert!(out.is_null());
}

#[test]
fn voices_pcm_refuses_each_null_out_slot() {
    let mut out_len: usize = 0;
    let status = unsafe { sillstone_audio_voices_pcm(1, 8, core::ptr::null_mut(), &mut out_len) };
    assert_eq!(status, SILLSTONE_E_INVALID, "null out");
    assert_eq!(out_len, 0);

    let mut out: *mut u8 = core::ptr::null_mut();
    let status = unsafe { sillstone_audio_voices_pcm(1, 8, &mut out, core::ptr::null_mut()) };
    assert_eq!(status, SILLSTONE_E_INVALID, "null out_len");
    assert!(out.is_null());
}

#[test]
fn index_add_and_match_split_argument_errors_from_rejections() {
    let pcm = mono_corpus();
    let idx = unsafe { sillstone_audio_index_new() };

    // Argument errors: bad geometry and bad channel counts.
    for (what, len, channels) in [("partial sample", 6usize, 1u32), ("zero channels", 4, 0)] {
        let status = unsafe { sillstone_audio_index_add(idx, pcm.as_ptr(), len, channels) };
        assert_eq!(status, SILLSTONE_E_INVALID, "index_add {what}");

        let mut s = Slots::new();
        let status = unsafe {
            sillstone_audio_match(idx, pcm.as_ptr(), len, channels, &mut s.out, &mut s.out_len)
        };
        assert_eq!(status, SILLSTONE_E_INVALID, "match {what}");
        s.assert_untouched(what);
    }

    // A partial interleaved frame passes the argument gate and is
    // refused by the signature stage: a rejection, the handle stays usable.
    let status = unsafe { sillstone_audio_index_add(idx, pcm.as_ptr(), 4, 2) };
    assert_eq!(status, SILLSTONE_E_REJECTED, "index_add partial frame");
    let mut s = Slots::new();
    let status =
        unsafe { sillstone_audio_match(idx, pcm.as_ptr(), 4, 2, &mut s.out, &mut s.out_len) };
    assert_eq!(status, SILLSTONE_E_REJECTED, "match partial frame");
    s.assert_untouched("match partial frame");

    // The rejected add left the index empty and alive: a valid query
    // against it succeeds with zero matches, and a valid add lands.
    let status = unsafe {
        sillstone_audio_match(idx, pcm.as_ptr(), pcm.len(), 1, &mut s.out, &mut s.out_len)
    };
    assert_eq!(status, SILLSTONE_OK);
    let stream = s.take();
    assert_eq!(be32(&stream, 8), 0, "match_count on the empty index");
    let status = unsafe { sillstone_audio_index_add(idx, pcm.as_ptr(), pcm.len(), 1) };
    assert_eq!(status, SILLSTONE_OK);
    let status = unsafe {
        sillstone_audio_match(idx, pcm.as_ptr(), pcm.len(), 1, &mut s.out, &mut s.out_len)
    };
    assert_eq!(status, SILLSTONE_OK);
    let stream = s.take();
    assert_eq!(be32(&stream, 8), 1, "the indexed corpus matches itself");
    assert_eq!(be32(&stream, 12), 0, "id of the first insertion");

    unsafe { sillstone_audio_index_free(idx) };
}
