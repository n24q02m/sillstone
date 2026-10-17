// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 sillstone

package sillstone

/*
#cgo CFLAGS: -I../../../include
#cgo LDFLAGS: -L../../../target/debug -L../../../target/release -lsillstone_ffi
#cgo windows LDFLAGS: -lws2_32 -luserenv -lntdll -lbcrypt
#include "sillstone.h"
#include <stdlib.h>
*/
import "C"
import (
	"encoding/binary"
	"fmt"
	"unsafe"
)

const (
	StatusOK       int32 = 0
	StatusInvalid  int32 = -1
	StatusRejected int32 = -2
)

var Formats = [...]string{"png", "jpeg", "bmp", "zip", "mp4", "mp3", "wav", "flac", "pdf", "gif", "unknown"}
var Modalities = [...]string{"image", "audio", "text", "binary", "video"}
var MatchTags = [...]string{"image", "audio", "text", "binary", "video"}
var signatureTags = [...]string{"image", "audio", "text", "binary", "video"}

type FfiError struct {
	Op     string
	Status int32
}

func (e *FfiError) Error() string {
	kind := "unknown failure"
	switch e.Status {
	case StatusInvalid:
		kind = "invalid argument"
	case StatusRejected:
		kind = "input rejected"
	}
	return fmt.Sprintf("%s failed: %s (status %d)", e.Op, kind, e.Status)
}

type Peak struct {
	T uint32
	F uint16
}

type Sig struct {
	Modality     string
	Phash        uint64
	PeakCount    int
	Frames       uint32
	Peaks        []Peak
	PeaksBytes   []byte
	WordCount    int
	Words        []uint64
	ChunkCount   int
	DigestBytes  []byte
	FrameCount   int
	Width        uint32
	Height       uint32
	DurationBits uint64
	FrameHashes  []uint64
	Minhash      []uint64
}

type Detection struct {
	Format   string
	Modality string
	Pending  bool
}

type Description struct {
	Format   string
	Modality string
	Tier1    string
	Sig      *Sig
	Facts    map[string]uint64
}

type MatchResult struct {
	Tag     string
	Slots   []uint64
	Matched bool
}

func Detect(data []byte) (Detection, error) {
	var format, modality, pending C.uint32_t
	var ptr *C.uint8_t
	if len(data) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}
	status := C.sillstone_curator_detect(ptr, C.size_t(len(data)), &format, &modality, &pending)
	if status != 0 {
		return Detection{}, &FfiError{Op: "sillstone_curator_detect", Status: int32(status)}
	}
	return Detection{
		Format:   Formats[format],
		Modality: Modalities[modality],
		Pending:  pending == 1,
	}, nil
}

func ContentHash(data []byte) ([]byte, error) {
	buf := make([]byte, 32)
	var outLen C.size_t
	var ptr *C.uint8_t
	if len(data) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}
	status := C.sillstone_curator_content_hash(ptr, C.size_t(len(data)), (*C.uint8_t)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &outLen)
	if status != 0 {
		return nil, &FfiError{Op: "sillstone_curator_content_hash", Status: int32(status)}
	}
	return buf[:outLen], nil
}

func SignatureStream(data []byte) ([]byte, error) {
	var outPtr *C.uint8_t
	var outLen C.size_t
	var ptr *C.uint8_t
	if len(data) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}
	status := C.sillstone_curator_signature(ptr, C.size_t(len(data)), &outPtr, &outLen)
	if status != 0 || outPtr == nil {
		return nil, &FfiError{Op: "sillstone_curator_signature", Status: int32(status)}
	}
	res := C.GoBytes(unsafe.Pointer(outPtr), C.int(outLen))
	C.sillstone_curator_free(outPtr, outLen)
	return res, nil
}

func DescribeStream(data []byte) ([]byte, error) {
	var outPtr *C.uint8_t
	var outLen C.size_t
	var ptr *C.uint8_t
	if len(data) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}
	status := C.sillstone_curator_describe(ptr, C.size_t(len(data)), &outPtr, &outLen)
	if status != 0 || outPtr == nil {
		return nil, &FfiError{Op: "sillstone_curator_describe", Status: int32(status)}
	}
	res := C.GoBytes(unsafe.Pointer(outPtr), C.int(outLen))
	C.sillstone_curator_free(outPtr, outLen)
	return res, nil
}

func Signature(data []byte) (*Sig, error) {
	raw, err := SignatureStream(data)
	if err != nil {
		return nil, err
	}
	return ParseSignature(raw)
}

func Describe(data []byte) (*Description, error) {
	raw, err := DescribeStream(data)
	if err != nil {
		return nil, err
	}
	return ParseDescribe(raw)
}

func MatchSignatures(a, b []byte) (MatchResult, error) {
	var tag, matched C.uint32_t
	slots := make([]C.uint64_t, 4)
	var ptrA, ptrB *C.uint8_t
	if len(a) > 0 {
		ptrA = (*C.uint8_t)(unsafe.Pointer(&a[0]))
	}
	if len(b) > 0 {
		ptrB = (*C.uint8_t)(unsafe.Pointer(&b[0]))
	}
	status := C.sillstone_curator_match(ptrA, C.size_t(len(a)), ptrB, C.size_t(len(b)), &tag, &slots[0], C.size_t(len(slots)), &matched)
	if status != 0 {
		return MatchResult{}, &FfiError{Op: "sillstone_curator_match", Status: int32(status)}
	}
	resSlots := make([]uint64, 4)
	for i, s := range slots {
		resSlots[i] = uint64(s)
	}
	return MatchResult{
		Tag:     MatchTags[tag],
		Slots:   resSlots,
		Matched: matched == 1,
	}, nil
}

func Fnv1a64(data []byte) uint64 {
	const offset = uint64(0xcbf29ce484222325)
	const prime = uint64(0x100000001b3)
	h := offset
	for _, b := range data {
		h ^= uint64(b)
		h *= prime
	}
	return h
}

func be32(b []byte, off int) uint32 {
	return binary.BigEndian.Uint32(b[off:])
}

func be64(b []byte, off int) uint64 {
	return binary.BigEndian.Uint64(b[off:])
}

func be16(b []byte, off int) uint16 {
	return binary.BigEndian.Uint16(b[off:])
}

func ParseSignature(raw []byte) (*Sig, error) {
	if len(raw) < 1 {
		return nil, fmt.Errorf("sillstone: signature stream is empty")
	}
	tag := raw[0]
	if int(tag) >= len(signatureTags) {
		return nil, fmt.Errorf("sillstone: unknown signature tag %d", tag)
	}
	modality := signatureTags[tag]
	switch modality {
	case "image":
		if len(raw) != 9 {
			return nil, fmt.Errorf("sillstone: image stream is not tag + u64")
		}
		return &Sig{Modality: modality, Phash: be64(raw, 1)}, nil
	case "audio":
		if len(raw) < 13 {
			return nil, fmt.Errorf("sillstone: audio stream is shorter than its 13-byte header")
		}
		peakCount := int(be64(raw, 1))
		frames := be32(raw, 9)
		payload := raw[13:]
		if peakCount != len(payload) || len(payload)%6 != 0 {
			return nil, fmt.Errorf("sillstone: audio peak count disagrees with the payload")
		}
		peaks := make([]Peak, 0, len(payload)/6)
		for i := 0; i < len(payload); i += 6 {
			peaks = append(peaks, Peak{T: binary.LittleEndian.Uint32(payload[i:]), F: binary.LittleEndian.Uint16(payload[i+4:])})
		}
		return &Sig{Modality: modality, PeakCount: peakCount, Frames: frames, Peaks: peaks, PeaksBytes: append([]byte(nil), payload...)}, nil
	case "text":
		if len(raw) < 9 {
			return nil, fmt.Errorf("sillstone: text stream is shorter than its 9-byte header")
		}
		wordCount := int(be64(raw, 1))
		payload := raw[9:]
		if wordCount != len(payload) || len(payload)%8 != 0 {
			return nil, fmt.Errorf("sillstone: text word count disagrees with the payload")
		}
		words := make([]uint64, 0, len(payload)/8)
		for i := 0; i < len(payload); i += 8 {
			words = append(words, be64(payload, i))
		}
		return &Sig{Modality: modality, WordCount: wordCount, Words: words}, nil
	case "binary":
		if len(raw) < 9 {
			return nil, fmt.Errorf("sillstone: binary stream is shorter than its 9-byte header")
		}
		chunkCount := int(be64(raw, 1))
		payload := raw[9:]
		if chunkCount != len(payload) || len(payload)%32 != 0 {
			return nil, fmt.Errorf("sillstone: binary chunk count disagrees with the payload")
		}
		return &Sig{Modality: modality, ChunkCount: chunkCount, DigestBytes: append([]byte(nil), payload...)}, nil
	default: // video
		if len(raw) < 21 {
			return nil, fmt.Errorf("sillstone: video stream is shorter than its 21-byte header")
		}
		frameCount := int(be32(raw, 1))
		width := be32(raw, 5)
		height := be32(raw, 9)
		durationBits := be64(raw, 13)
		rest := raw[21:]
		if frameCount > len(rest) || frameCount%8 != 0 {
			return nil, fmt.Errorf("sillstone: video frame count disagrees with the payload")
		}
		framesPayload := rest[:frameCount]
		minhashPayload := rest[frameCount:]
		if len(minhashPayload)%8 != 0 {
			return nil, fmt.Errorf("sillstone: video minhash payload is not whole words")
		}
		frameHashes := make([]uint64, 0, len(framesPayload)/8)
		for i := 0; i < len(framesPayload); i += 8 {
			frameHashes = append(frameHashes, binary.LittleEndian.Uint64(framesPayload[i:]))
		}
		minhash := make([]uint64, 0, len(minhashPayload)/8)
		for i := 0; i < len(minhashPayload); i += 8 {
			minhash = append(minhash, binary.LittleEndian.Uint64(minhashPayload[i:]))
		}
		return &Sig{
			Modality:     modality,
			FrameCount:   frameCount,
			Width:        width,
			Height:       height,
			DurationBits: durationBits,
			FrameHashes:  frameHashes,
			Minhash:      minhash,
		}, nil
	}
}

func factsLen(modality string) int {
	switch modality {
	case "image":
		return 16
	case "audio":
		return 18
	case "text":
		return 16
	case "binary":
		return 16
	default:
		return 24
	}
}

func ParseDescribe(raw []byte) (*Description, error) {
	if len(raw) < 35 {
		return nil, fmt.Errorf("sillstone: describe stream is shorter than its 35-byte header")
	}
	fmtCode, modCode := raw[0], raw[1]
	if int(fmtCode) >= len(Formats) || int(modCode) >= len(Modalities) {
		return nil, fmt.Errorf("sillstone: unknown describe codes %d/%d", fmtCode, modCode)
	}
	tier1 := fmt.Sprintf("%x", raw[2:34])
	modality := Modalities[modCode]
	sigEnd := len(raw) - factsLen(modality)
	sig, err := ParseSignature(raw[34:sigEnd])
	if err != nil {
		return nil, err
	}
	factsRaw := raw[sigEnd:]
	facts := map[string]uint64{}
	need := func(n int) error {
		if len(factsRaw) != n {
			return fmt.Errorf("sillstone: facts block is %d bytes, want %d", len(factsRaw), n)
		}
		return nil
	}
	switch modality {
	case "image":
		if err := need(16); err != nil {
			return nil, err
		}
		facts["width"] = uint64(be32(factsRaw, 0))
		facts["height"] = uint64(be32(factsRaw, 4))
		facts["keypoints"] = be64(factsRaw, 8)
	case "audio":
		if err := need(18); err != nil {
			return nil, err
		}
		facts["sample_rate"] = uint64(be32(factsRaw, 0))
		facts["channels"] = uint64(be16(factsRaw, 4))
		facts["frames"] = uint64(be32(factsRaw, 6))
		facts["peaks"] = be64(factsRaw, 10)
	case "text":
		if err := need(16); err != nil {
			return nil, err
		}
		facts["words"] = be64(factsRaw, 0)
		facts["canonical_len"] = be64(factsRaw, 8)
	case "binary":
		if err := need(16); err != nil {
			return nil, err
		}
		facts["len"] = be64(factsRaw, 0)
		facts["chunks"] = be64(factsRaw, 8)
	default: // video
		if err := need(24); err != nil {
			return nil, err
		}
		facts["width"] = uint64(be32(factsRaw, 0))
		facts["height"] = uint64(be32(factsRaw, 4))
		facts["duration_bits"] = be64(factsRaw, 8)
		facts["frames_sampled"] = be64(factsRaw, 16)
	}
	return &Description{Format: Formats[fmtCode], Modality: modality, Tier1: tier1, Sig: sig, Facts: facts}, nil
}
