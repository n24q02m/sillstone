# CHANGELOG

<!-- version list -->

## v0.4.1-beta.1 (2026-11-17)

### Bug Fixes

- Align renovate.json with the fleet dependency policy
  ([#97](https://github.com/n24q02m/sillstone/pull/97),
  [`c91028f`](https://github.com/n24q02m/sillstone/commit/c91028f3afab5c41a412e76512c803370dec7d4e))

- Bring CI workflows in line with the fleet CI policy lint
  ([#96](https://github.com/n24q02m/sillstone/pull/96),
  [`1287efb`](https://github.com/n24q02m/sillstone/commit/1287efbb62ebccf2e962addb60857e5db4ea2af3))

- Bring sillstone up to the tier-1 repository standard
  ([#98](https://github.com/n24q02m/sillstone/pull/98),
  [`68e351c`](https://github.com/n24q02m/sillstone/commit/68e351c3ad904f8b4794ddfdb51459dac071011f))

- Compute the release version with better-semantic-release
  ([#99](https://github.com/n24q02m/sillstone/pull/99),
  [`86d4dce`](https://github.com/n24q02m/sillstone/commit/86d4dce3745b1a98157768f59e9ade2e0801361a))

- Declare the main branch ruleset as a ruleset file
  ([#92](https://github.com/n24q02m/sillstone/pull/92),
  [`0f58044`](https://github.com/n24q02m/sillstone/commit/0f5804440fc4a3d03466b3fa79a577ded0710727))

- Drop the skret sync target that duplicates the central secret declaration
  ([#93](https://github.com/n24q02m/sillstone/pull/93),
  [`1c7b2aa`](https://github.com/n24q02m/sillstone/commit/1c7b2aac96a70bd96371685f49516b55fb52a580))

- Fold bench into ci.yml and add the standard repository files
  ([#95](https://github.com/n24q02m/sillstone/pull/95),
  [`2dcda6d`](https://github.com/n24q02m/sillstone/commit/2dcda6d35dddfeb9dd5788d9967eb50206e689d3))

### Chores

- **deps**: Update dependency numpy to v2.5.5 ([#94](https://github.com/n24q02m/sillstone/pull/94),
  [`5f23fdd`](https://github.com/n24q02m/sillstone/commit/5f23fdd6126ec767a50f2196ebfd2200a320d8a7))


## v0.4.0 (2026-11-14)

### Bug Fixes

- Resolve CI matrices from upstream support windows and measure every binding
  ([#88](https://github.com/n24q02m/sillstone/pull/88), [`e877a12`](https://github.com/n24q02m/sillstone/commit/e877a1244010f39dd1285ca935484c1d257d28f1))

- Cut 0.4.0-beta.1 for the pith to sillstone rename
  ([#90](https://github.com/n24q02m/sillstone/pull/90), [`16e62cf`](https://github.com/n24q02m/sillstone/commit/16e62cff8f75be8000e2cb70570f52074a0bae58))

- Cut 0.4.0 stable for the pith to sillstone rename
  ([#91](https://github.com/n24q02m/sillstone/pull/91), [`2e7ca9a`](https://github.com/n24q02m/sillstone/commit/2e7ca9a37a131c29d24aa692100c615eb66f2c97))

### Features

- Finish the pith to sillstone rename with deprecated aliases
  ([#89](https://github.com/n24q02m/sillstone/pull/89), [`b6a24b5`](https://github.com/n24q02m/sillstone/commit/b6a24b5ad0f28a6f7ceff3a1177faa461f3de3fa))

## v0.3.2 (2026-11-13)

### Bug Fixes

- Publish Java package automatically and gate every registry
  ([#83](https://github.com/n24q02m/sillstone/pull/83), [`317a21f`](https://github.com/n24q02m/sillstone/commit/317a21fd5f2a448960e4c0c6126f11c96d70b012))

- Cut 0.3.2-beta.1 to republish the Java artifact with its native prebuilds
  ([#84](https://github.com/n24q02m/sillstone/pull/84), [`e4d8bbe`](https://github.com/n24q02m/sillstone/commit/e4d8bbe0ea468a0f870182fe662b88d99a38f3b0))

- Stage Java prebuilds before the Maven rebuild
  ([#85](https://github.com/n24q02m/sillstone/pull/85), [`b7734eb`](https://github.com/n24q02m/sillstone/commit/b7734eb99f225a00fc32931b82d4b1f4dbc4688f))

- Give the Maven readback an hour and stream gate output
  ([#86](https://github.com/n24q02m/sillstone/pull/86), [`436c4d7`](https://github.com/n24q02m/sillstone/commit/436c4d7ec707aef78f99d785d70a323f76418e65))

- Cut 0.3.2 stable with the corrected Java artifact
  ([#87](https://github.com/n24q02m/sillstone/pull/87), [`b68b708`](https://github.com/n24q02m/sillstone/commit/b68b708ab77f98ad8cbe2aade599da4a5321a39f))

## v0.3.1 (2026-11-13)

### Bug Fixes

- Guard null JNIEnv in byte_array and bind scalar out slot with as_mut
  ([#16](https://github.com/n24q02m/sillstone/pull/16), [`51136ce`](https://github.com/n24q02m/sillstone/commit/51136ce96fc1a41a8759f595150c32aa3b8185cc))

- Fail CD on auditwheel errors and measure coverage in CI
  ([#17](https://github.com/n24q02m/sillstone/pull/17), [`1ed236d`](https://github.com/n24q02m/sillstone/commit/1ed236d174afede597a734db957fe7b87b3effbc))

- Report measured line coverage in scorecard instead of unmeasured pass claims
  ([#18](https://github.com/n24q02m/sillstone/pull/18), [`005c929`](https://github.com/n24q02m/sillstone/commit/005c9295bde8a21970bc19a5389ea9ed73e7f5a8))

- Make resize, reduce and convert bit-exact with Pillow
  ([#19](https://github.com/n24q02m/sillstone/pull/19), [`a171622`](https://github.com/n24q02m/sillstone/commit/a171622a57579c7bca108a9b4f53a76109c72044))

- Make release CD report only what it published and ship npm docs
  ([#20](https://github.com/n24q02m/sillstone/pull/20), [`b2b8699`](https://github.com/n24q02m/sillstone/commit/b2b86996dc71f205b1e188ac3b11c501e7b24490))

- Bump version to 0.3.1-beta.1
  ([#22](https://github.com/n24q02m/sillstone/pull/22), [`cfbec51`](https://github.com/n24q02m/sillstone/commit/cfbec51fa56a32e9094166a86b879bee13154c23))

- Stop reference generator tests from rewriting the committed reference.json
  ([#23](https://github.com/n24q02m/sillstone/pull/23), [`8e460c8`](https://github.com/n24q02m/sillstone/commit/8e460c8e08ef8de3b96b9b56fd3df0a33290a3db))

- Pin the golden oracle to numpy 2.5.4 and regenerate provenance
  ([#25](https://github.com/n24q02m/sillstone/pull/25), [`da08c77`](https://github.com/n24q02m/sillstone/commit/da08c770b0cc0d2f9269ede481a960b5f7c8fe7c))

- Measure FFI and JNI coverage by profiling the SDK suites against an instrumented library
  ([#24](https://github.com/n24q02m/sillstone/pull/24), [`d5bf6fc`](https://github.com/n24q02m/sillstone/commit/d5bf6fc456341e7638f57fdab2f4062bc44a1723))

- Tensor reductions read only the view's elements, zero-sized tensors no longer panic
  ([#26](https://github.com/n24q02m/sillstone/pull/26), [`142ca52`](https://github.com/n24q02m/sillstone/commit/142ca520546643bb06df987ff7a04fbe82bd6bcf))

- Make publish step idempotent with explicit labels and npm E404 check
  ([#27](https://github.com/n24q02m/sillstone/pull/27), [`bed6991`](https://github.com/n24q02m/sillstone/commit/bed6991a03e7cd8d16cf0704f71d56df514ac885))

- Replace stale pith naming in crate descriptions
  ([#28](https://github.com/n24q02m/sillstone/pull/28), [`c099123`](https://github.com/n24q02m/sillstone/commit/c0991230b6d930f8b2a8c5db48cb3f39c7397a26))

- Refuse brotli and zstd payloads the decoders cannot decode
  ([#29](https://github.com/n24q02m/sillstone/pull/29), [`4c89e71`](https://github.com/n24q02m/sillstone/commit/4c89e718ff3a20aededc0508353a676374e55a9e))

- **digest**: Accept null out for zero-length BLAKE3 XOF and add reference-vector tests
  ([#32](https://github.com/n24q02m/sillstone/pull/32), [`9e19d6f`](https://github.com/n24q02m/sillstone/commit/9e19d6f1e130d0e29cbd5f9abd885ca1eeb37ef8))

- **qoi**: Bound decode allocation by stream size and return an error on encode overflow
  ([#33](https://github.com/n24q02m/sillstone/pull/33), [`9be72ff`](https://github.com/n24q02m/sillstone/commit/9be72ff4a0d6de257101f0a5981261ab049354d0))

- **image**: Bound-check FFI geometry and keep alpha in RGBA BMP
  ([#35](https://github.com/n24q02m/sillstone/pull/35), [`1dfbf1c`](https://github.com/n24q02m/sillstone/commit/1dfbf1c306ce866277840facb3e5c81fa7c69d18))

- **mp3**: Correct ID3 APIC/unsync/frame-flag handling; test remaining math FFI
  ([#36](https://github.com/n24q02m/sillstone/pull/36), [`ee88f45`](https://github.com/n24q02m/sillstone/commit/ee88f45b6d07850b99178c400a5b3627900529a0))

- **python**: Open 32-bit BMP, verify BMP alpha against Pillow; checked crop size
  ([#37](https://github.com/n24q02m/sillstone/pull/37), [`d9e871d`](https://github.com/n24q02m/sillstone/commit/d9e871d94afa130757a828087d0e77c3644bd7e3))

- **file**: Parse PAX records by length and harden TAR size and path handling
  ([#38](https://github.com/n24q02m/sillstone/pull/38), [`6f8aba1`](https://github.com/n24q02m/sillstone/commit/6f8aba1c35d85845d934a70d0540ffee17258762))

- **mp4**: Resolve fragmented sample offsets against the moof box and bound untrusted counts
  ([#39](https://github.com/n24q02m/sillstone/pull/39), [`f21897d`](https://github.com/n24q02m/sillstone/commit/f21897d884e6319ced635d40a3d995fecf3a6ff5))

- **tensor**: Propagate NaN through min/max/argmax, add argmin and float mean, size zero-sized sums exactly
  ([#41](https://github.com/n24q02m/sillstone/pull/41), [`1f12208`](https://github.com/n24q02m/sillstone/commit/1f12208932db9946b97640737ce76ca30fcbe889))

- **python**: Make Array follow NumPy semantics for dtypes, indexing, promotion and reductions
  ([#42](https://github.com/n24q02m/sillstone/pull/42), [`70a775c`](https://github.com/n24q02m/sillstone/commit/70a775cfc35674b4006dd979e110f820fc9b48ac))

- **python**: Allow out-of-range Python ints as true-division operands
  ([#43](https://github.com/n24q02m/sillstone/pull/43), [`c291e2b`](https://github.com/n24q02m/sillstone/commit/c291e2b03672733fe05a93a2803b978f91d2ccbb))

- **python**: Validate BMP header geometry before allocating the raster
  ([#44](https://github.com/n24q02m/sillstone/pull/44), [`5e03fe7`](https://github.com/n24q02m/sillstone/commit/5e03fe74c20180c62d13ee8154e5d5ad4ca0a9ad))

- **ffi**: Reject TLSH digests that are not 72 bytes in the JNI bridge and test the bridge against a fake JNIEnv
  ([#47](https://github.com/n24q02m/sillstone/pull/47), [`b0bdf28`](https://github.com/n24q02m/sillstone/commit/b0bdf2870bb7c1bb2c41e48be56c59a0c4fb032d))

- **ffi**: Keep JNI shim symbols out of the test binary so their coverage is counted
  ([#50](https://github.com/n24q02m/sillstone/pull/50), [`dd61764`](https://github.com/n24q02m/sillstone/commit/dd617643dad854e660a89e890f4f041d28ebb15f))

- **python**: Make Image.putpixel, __array_interface__ and from_array work
  ([#46](https://github.com/n24q02m/sillstone/pull/46), [`3a1157c`](https://github.com/n24q02m/sillstone/commit/3a1157ca2d7ebf7f453b57457725752f4deec39c))

- **file**: Apply a PAX global size record to every entry after it
  ([#45](https://github.com/n24q02m/sillstone/pull/45), [`58a488a`](https://github.com/n24q02m/sillstone/commit/58a488a6e0fe596eab6ff3aba3f588170d4790d9))

- **curator**: Test the renamed JNI exports the published Java classes bind to
  ([#52](https://github.com/n24q02m/sillstone/pull/52), [`5a24e0f`](https://github.com/n24q02m/sillstone/commit/5a24e0f44be8d0f20a330d940a76e617883d4d0a))

- **file**: Pin and document how a PAX global size reaches directories and symlinks
  ([#61](https://github.com/n24q02m/sillstone/pull/61), [`6805fed`](https://github.com/n24q02m/sillstone/commit/6805fed6b0d7fa8158f1448c67a454f768407097))

- **file**: Keep FFI and JNI symbols out of the unit-test binary so their coverage is counted
  ([#53](https://github.com/n24q02m/sillstone/pull/53), [`b9112f1`](https://github.com/n24q02m/sillstone/commit/b9112f1b112d3f87ab7e6abf550b7db5e7783923))

- **flac**: Keep FFI symbols out of the unit-test binary so their coverage is counted
  ([#54](https://github.com/n24q02m/sillstone/pull/54), [`b10f2a1`](https://github.com/n24q02m/sillstone/commit/b10f2a1fafc0b72b47c5dd5189558d867bff8ca7))

- **pdf**: Keep FFI symbols out of the unit-test binary so their coverage is counted
  ([#55](https://github.com/n24q02m/sillstone/pull/55), [`717b516`](https://github.com/n24q02m/sillstone/commit/717b516e4a99267cf9da4416d9a0efa7519adc98))

- **video**: Keep FFI symbols out of the unit-test binary so their coverage is counted
  ([#56](https://github.com/n24q02m/sillstone/pull/56), [`84015da`](https://github.com/n24q02m/sillstone/commit/84015dacce1217efc4b78fbc10804f155e0fa1cd))

- **zip**: Keep FFI symbols out of the unit-test binary so their coverage is counted
  ([#57](https://github.com/n24q02m/sillstone/pull/57), [`c857369`](https://github.com/n24q02m/sillstone/commit/c85736919171df421b4af3a575a3db54f7d64143))

- **mp3**: Keep FFI symbols out of the unit-test binary so their coverage is counted
  ([#58](https://github.com/n24q02m/sillstone/pull/58), [`02e20d3`](https://github.com/n24q02m/sillstone/commit/02e20d37c5ccdb28b5295e8de7cde67b087c989c))

- **unicode**: Test the grapheme count and category exports and count their coverage
  ([#60](https://github.com/n24q02m/sillstone/pull/60), [`c1747a2`](https://github.com/n24q02m/sillstone/commit/c1747a22bca802be640eac2026889707facb81bf))

- **image**: Test QOI decoding when the stream channel count differs from the requested layout
  ([#63](https://github.com/n24q02m/sillstone/pull/63), [`8966de5`](https://github.com/n24q02m/sillstone/commit/8966de5bcf851918ddf88f06b9d09d4a79925e46))

- **pdf**: Test AcroForm field types, values and hierarchies
  ([#64](https://github.com/n24q02m/sillstone/pull/64), [`942084e`](https://github.com/n24q02m/sillstone/commit/942084e9c46bfa98a85c7ee422ca1ec822e1f1b4))

- **cdc**: Test Buzhash64 against independently computed fingerprints
  ([#65](https://github.com/n24q02m/sillstone/pull/65), [`d5c2335`](https://github.com/n24q02m/sillstone/commit/d5c23350db6d5321e67359b7ca453db8dfc119f3))

- **math**: Test FFI and JNI refusals, DST JNI entries and failed array allocation
  ([#66](https://github.com/n24q02m/sillstone/pull/66), [`fe7bf9f`](https://github.com/n24q02m/sillstone/commit/fe7bf9f2cb0a01b9a37b1ae9b166771b06d6e77a))

- **inflate**: Test the gzip, Brotli and Zstandard C exports
  ([#67](https://github.com/n24q02m/sillstone/pull/67), [`5d8d6eb`](https://github.com/n24q02m/sillstone/commit/5d8d6eb3a006df33254ce32cb9ec52fb86486005))

- **audio**: Test C ABI refusals, the PCM signature path and JNI null-array reports
  ([#68](https://github.com/n24q02m/sillstone/pull/68), [`0f0ce01`](https://github.com/n24q02m/sillstone/commit/0f0ce015f48f9ddd2418208005ad7844025e28e7))

- **text**: Test the refusal paths of the text C ABI and JNI glue
  ([#69](https://github.com/n24q02m/sillstone/pull/69), [`88fcc2b`](https://github.com/n24q02m/sillstone/commit/88fcc2b69c8de08af5b008441a7a5191f751c4b7))

- Cover image C ABI refusals the surface tests missed
  ([#70](https://github.com/n24q02m/sillstone/pull/70), [`c5d5eb4`](https://github.com/n24q02m/sillstone/commit/c5d5eb485b3c2bf8ff851ca7763701faeb93dad8))

- **digest**: Test C ABI refusals, the PRNG fill paths and JNI unusable arrays
  ([#71](https://github.com/n24q02m/sillstone/pull/71), [`344568e`](https://github.com/n24q02m/sillstone/commit/344568ed3d81be618b66b7d46b9da8ced275cab0))

- Decouple Java and Go publishing from npm in cd.yml
  ([`d2b9047`](https://github.com/n24q02m/sillstone/commit/d2b90470def89680597669c7d19cdcc38ee6a088))

- Rename cli binary and references to sillstone
  ([`8abb995`](https://github.com/n24q02m/sillstone/commit/8abb9957b61ea9a1ec299fc60a59083294549386))

- Fail wheel repair and gate the measured coverage target
  ([#72](https://github.com/n24q02m/sillstone/pull/72), [`e4ed2fd`](https://github.com/n24q02m/sillstone/commit/e4ed2fdfa2fd8db7df00246e1d1bb4e2ee0ddf68))

- Build the python sdist from source instead of shipping a broken archive
  ([#73](https://github.com/n24q02m/sillstone/pull/73), [`962520d`](https://github.com/n24q02m/sillstone/commit/962520dc886dc38747eca5c0d9170e49703c7189))

- Match Pillow's convert luma and float resampling arithmetic
  ([#75](https://github.com/n24q02m/sillstone/pull/75), [`0e734be`](https://github.com/n24q02m/sillstone/commit/0e734be1009cf38376770643e516202305501adf))

- Accept a metadata meta-block as the last one and refuse brotli trailing bytes
  ([#79](https://github.com/n24q02m/sillstone/pull/79), [`5d4928e`](https://github.com/n24q02m/sillstone/commit/5d4928e1556d8863b3e2394c67fb4c320e7d8f59))

- Move the release manifests to the next beta
  ([#80](https://github.com/n24q02m/sillstone/pull/80), [`7d2ce4a`](https://github.com/n24q02m/sillstone/commit/7d2ce4a75e4931fc07561cf7dd6ede081d2241cf))

- Give the npm readiness check a 20-minute window
  ([#81](https://github.com/n24q02m/sillstone/pull/81), [`a9ba688`](https://github.com/n24q02m/sillstone/commit/a9ba688a34a36022e952cdd17bc645200a6c19b2))

- Move the release manifests to 0.3.1
  ([#82](https://github.com/n24q02m/sillstone/pull/82), [`5e2c2bb`](https://github.com/n24q02m/sillstone/commit/5e2c2bb0b8950f7c8f5f07d109b2574b7c99086e))

### Features

- **inflate**: Decode Zstandard Huffman literals and FSE sequences (RFC 8878)
  ([#30](https://github.com/n24q02m/sillstone/pull/30), [`e3a994b`](https://github.com/n24q02m/sillstone/commit/e3a994b8fde99c54d4f397ed9d6e4bd97813ee8c))

- **inflate**: Decode Brotli compressed meta-blocks (RFC 7932)
  ([#31](https://github.com/n24q02m/sillstone/pull/31), [`aad4d18`](https://github.com/n24q02m/sillstone/commit/aad4d18b38f4e9992e7cb9c6771580bb70f96207))

- **java**: Prepare Maven Central Portal publishing under io.github.n24q02m
  ([#34](https://github.com/n24q02m/sillstone/pull/34), [`8d5e79d`](https://github.com/n24q02m/sillstone/commit/8d5e79d2732709f4ec081a5df3c297c4011879e8))

- **mp4**: Test the C ABI record walk for audio, other entries and null arguments
  ([#40](https://github.com/n24q02m/sillstone/pull/40), [`da19f31`](https://github.com/n24q02m/sillstone/commit/da19f312bbb4e7f6eaa42fcbfc30607031908f66))

- **ci**: Publish per-line Rust coverage as an lcov file in the coverage artifact
  ([#48](https://github.com/n24q02m/sillstone/pull/48), [`5a262bb`](https://github.com/n24q02m/sillstone/commit/5a262bb1b8a9f0db7ae748865489656bd6f61ead))

- **abi**: Add sillstone_text_tlsh_distance_checked and use it from the Python, Node and Go wrappers
  ([#62](https://github.com/n24q02m/sillstone/pull/62), [`56aa8d8`](https://github.com/n24q02m/sillstone/commit/56aa8d816c57849ab0ffa44326828dff223063a0))

- Derive the ci language matrices from the declared floors
  ([#74](https://github.com/n24q02m/sillstone/pull/74), [`d546df0`](https://github.com/n24q02m/sillstone/commit/d546df069e7466bcd6411e4b7a4e794f888cccbd))

- Complete the tensor layer's NumPy parity and stop copying payloads
  ([#76](https://github.com/n24q02m/sillstone/pull/76), [`dd1813e`](https://github.com/n24q02m/sillstone/commit/dd1813e378fa753fcdb6c6fa15123009fedbc58f))

- **inflate**: Add zstd vectors generated from the reference encoder
  ([#77](https://github.com/n24q02m/sillstone/pull/77), [`953b2ff`](https://github.com/n24q02m/sillstone/commit/953b2ff9926e814df85d612674a477a42b88842b))

- Build the python sdist from source and verify the install in ci
  ([#78](https://github.com/n24q02m/sillstone/pull/78), [`47224ec`](https://github.com/n24q02m/sillstone/commit/47224ec8fa2e197051ba99844c7684d16fd7e2cb))

## v0.3.0 (2026-10-28)

### Bug Fixes

- Fail fast on PyPI publish errors without swallowing failures
  ([`12a8a87`](https://github.com/n24q02m/sillstone/commit/12a8a87e5068120db07e656becea6feff14b888d))

- Bump default expected_version to 0.2.1 in cd.yml
  ([`5269b9a`](https://github.com/n24q02m/sillstone/commit/5269b9a27571cf54a0a33d63cdc5c65e806601fc))

- Set skip-existing true in pypi publish step
  ([`e29f8e4`](https://github.com/n24q02m/sillstone/commit/e29f8e465c703319aba077a7b8277f280b9dd772))

- Resolve workspace clippy errors blocking CI on main
  ([`bdec4bc`](https://github.com/n24q02m/sillstone/commit/bdec4bcb5e9b5a89a01e53a1bfe018017a4522f2))

- Install Pillow and NumPy oracles for Python SDK parity tests in CI
  ([`7198197`](https://github.com/n24q02m/sillstone/commit/7198197bd57610900badf39af70b64ce8ee43b3b))

- Build N-API image wrappers and ship the addon in npm releases
  ([`a43d8d8`](https://github.com/n24q02m/sillstone/commit/a43d8d8c92d84edae24b493215e53ad4c42bf6ba))

- Gate pre-release readback on all crates, strict npm contents and propagation waits, and tag releases from CD
  ([`c71b92b`](https://github.com/n24q02m/sillstone/commit/c71b92bb077fdbb274a69fe17ecb173104f62fef))

- Print the whash intermediate digest in the pywt probe
  ([`6464eeb`](https://github.com/n24q02m/sillstone/commit/6464eebc5b184b117d21be9e081e67692d4e7432))

- Keep the whash Haar transform from contracting into fused multiply-adds
  ([`25810be`](https://github.com/n24q02m/sillstone/commit/25810be3a69ea138cf790e907ed74442ae778ed0))

- Compare whash only against PyWavelets builds that round Haar taps separately
  ([`35cc200`](https://github.com/n24q02m/sillstone/commit/35cc200af9733011a846efff3ca555ce8b857e0f))

- Check the Go proxy against the sdk/go module path
  ([`ad6253e`](https://github.com/n24q02m/sillstone/commit/ad6253e17297c95f78367ac02fef1fe9a203cc47))

- Switch npm publish to Trusted Publisher OIDC
  ([`3956d77`](https://github.com/n24q02m/sillstone/commit/3956d7787efcbc4df0a93d03df108ab7062e5155))

### Features

- Implement Pillow and NumPy replacement engines with omnibus SDK facades
  ([`7639235`](https://github.com/n24q02m/sillstone/commit/7639235703227f01f7d5a8b278f6277e14759163))

- Add tensor FFI acceleration and parity benchmark suite
  ([`8f846e0`](https://github.com/n24q02m/sillstone/commit/8f846e056f6fa72bfbf40beb87e52af29b4eb28d))

- Add hashlib and imagehash parity benchmark and fix whash/colorhash bindings
  ([`14bf083`](https://github.com/n24q02m/sillstone/commit/14bf0831c3abb2f736e6db355696fbcc3341dd1a))

- Bump to 0.3.0-beta.1 and gate releases on manifest version consistency
  ([`178b838`](https://github.com/n24q02m/sillstone/commit/178b838dfd2ea2ff0001c08bc84bb43f0bf0f30c))

- Make image hashes bit-exact with imagehash and Pillow
  ([`757e58c`](https://github.com/n24q02m/sillstone/commit/757e58c163ce47a794db47b059eed168e888d5ea))

- Speed up digests and image resize with runtime-detected SIMD kernels
  ([`1fedfdb`](https://github.com/n24q02m/sillstone/commit/1fedfdbf0444abf0dcacc0f3e6491bd1ac42e3f7))

- Add image and array facades to the Node and Java SDKs over the native core
  ([`ab6abc6`](https://github.com/n24q02m/sillstone/commit/ab6abc64df2f35d26c895e958365b9bef94fb5cf))

- Speed up the image hashes with SIMD luma and madd-based Lanczos rows
  ([`98a1089`](https://github.com/n24q02m/sillstone/commit/98a1089931bb3ca0d35ec9bf1c818ce23be51b8b))

- Skip ctypes argument conversion on the Python digest entry points
  ([`2bc07b8`](https://github.com/n24q02m/sillstone/commit/2bc07b869acd561c5c35e411691d6475ae5fe734))

- Add an ARMv8.2 SHA-3 Keccak kernel behind runtime detection
  ([`aa00711`](https://github.com/n24q02m/sillstone/commit/aa00711171c5e5f88d6ee7182d9e08065a6a85ff))

- Unroll the SHA-256 extension kernels so the schedule stays in registers
  ([`18ae161`](https://github.com/n24q02m/sillstone/commit/18ae16107448de1d7e861679b0b1773633e90a60))

- Unroll the BLAKE2b AVX2 rounds with constant message indices
  ([`47bf41a`](https://github.com/n24q02m/sillstone/commit/47bf41aba8d9c2944b36cf091c9b332d77b3293c))

- Add the Go image operations and native tensor facade
  ([`7292f4d`](https://github.com/n24q02m/sillstone/commit/7292f4d5d43eb9d095736bf51fa816726e94a8a1))

- Document measured speed parity, the whash oracle caveat and the facade limits
  ([`f8afdc0`](https://github.com/n24q02m/sillstone/commit/f8afdc013fa3de9f0598106402783c7e60956f18))

- Bump the release version to 0.3.0-beta.2
  ([`3df29e6`](https://github.com/n24q02m/sillstone/commit/3df29e6cf20cf491109f825339f9dbca915aa347))

- Bump version manifests to 0.3.0
  ([`2b85bc3`](https://github.com/n24q02m/sillstone/commit/2b85bc30f7377c752e8bb9a42ae5e6506d0b7237))

## v0.2.1 (2026-10-19)

- First release of the Sillstone monorepo, which replaces the standalone `pith-*`
  packages; [docs/MIGRATION.md](docs/MIGRATION.md) maps each old package to its
  new crate and SDK.
