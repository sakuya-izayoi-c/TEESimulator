# third_party

Everything TEESimulator depends on but doesn't author. All of it is a git submodule
(`git submodule update --init --recursive`), pinned to a specific upstream commit so a
build is reproducible. Nothing here is edited in git — except the build-time
patches called out below.

| Directory | Upstream | Why it's here |
|---|---|---|
| `keymint` | [platform/system/keymint](https://android.googlesource.com/platform/system/keymint) | AOSP's reference KeyMint TA (`kmr-ta`) and its BoringSSL crypto backend (`kmr-crypto-boring`). The Rust TA in `../rust/teesim-km` embeds this — it *is* the software KeyMint Cuttlefish runs. **Patched at build time** (see below). |
| `boringssl` | [boringssl](https://boringssl.googlesource.com/boringssl) (shallow) | The crypto library. Its headers feed the Rust `openssl-sys` bindings on every platform; on Android 10/11 its static `crypto` target is built internally (`add_subdirectory` from the top-level `CMakeLists.txt`) and bundled into the keystore interceptor. |
| `interfaces` | [platform/hardware/interfaces](https://android.googlesource.com/platform/hardware/interfaces) | The KeyMint / secureclock / sharedsecret **AIDL**. `CMakeLists.txt` runs `aidl --lang=ndk` over these to generate the `Bn…`/`Bp…` stubs the keymint interceptor is built from. |
| `frameworks-native` | [platform/frameworks/native](https://android.googlesource.com/platform/frameworks/native) | libbinder. The keymint interceptor compiles the NDK binder headers from `libs/binder/ndk`; the C++ binder headers also live here (see the `keystore/aosp` note). |
| `lsplt` | [JingMatrix/LSPlt](https://github.com/JingMatrix/LSPlt) | PLT/GOT hooking and `/proc/<pid>/maps` parsing — used by the injector and the keymint `AIBinder_transact` hook. |

## Build-time patches

`rust/patches/` holds the delta between the reference TA as AOSP builds it and what we
need it to be. The active patches cover build integration and attestation encoding.
User authentication and keyblob version binding use the reference implementation.

| Patch | What it changes |
|---|---|
| `kmr-crypto-boring` | The crate is written to build under AOSP's **Soong** against `bssl-sys` (Soong's BoringSSL binding). We build under **Cargo** against `openssl-sys`, so this drops the `#[cfg(soong)]` paths and an `i32` key-length narrowing the `openssl-sys` binding doesn't need. |
| `kmr-crypto-boring-ec-group` | Upstream parses an imported EC private key without naming its curve, which only works with Android's `rust-openssl` fork. Reaches for BoringSSL's group-aware entry point instead, so a SEC1 key that legally omits its parameters still parses — and one naming the wrong curve is rejected. |
| `kmr-ta-seclevel` | Lets a single TA attest at the security level and `attestationVersion` of the HAL a request arrived through, so a dual-level device stays honest at both, and keeps pre-KeyMint (Keymaster) attestation versions representable. |
| `kmr-ta-versioned-rot` | Restricts the local extension encoder to its supported wire formats. |

The earlier exceptions for unverifiable auth-token MACs and downward keyblob
re-stamping were removed. A profile with a lower patch level cannot reopen a
keyblob bound to a higher level by relabeling it. `rust/verify-stock-security.sh`
checks the three security-critical source files before compilation; if an old
patched tree fails this check, preserve any local edits and use a clean checkout.

`rust/build.sh` applies them to the `keymint` submodule's **working tree** at build
time, idempotently: each is reverse-checked first (`git apply --reverse --check`) and
only applied if it isn't already there. Consequence: after a build, `git status` shows
`third_party/keymint` as *modified* — that's the applied patches, not a mistake, and
they never get committed. Reset it with `git -C third_party/keymint checkout .` if a
clean submodule is needed.

A change made directly in the submodule's working tree is invisible to git here and
that reset throws it away, so anything worth keeping has to land in `rust/patches/`
before the tree is cleaned.

If the dirty-submodule friction ever gets annoying, the alternative is to fork
`system/keymint`, commit the patches there, and repoint this submodule at the fork —
then the build needs no patch step at all. This file (and the `.patch` files) stay the
record of exactly what changed.

## `keystore/aosp`

The Android 10/11 interceptor talks to the legacy keystore over the **C++** libbinder,
which the NDK doesn't ship. It needs two sets of platform headers: binder (from
`frameworks/native`, which *is* a submodule here) and libutils — `RefBase.h`,
`String8.h`, `Vector.h`, … — which live in `system/core`, a large repo we don't want to
submodule for a dozen headers. So `keystore/aosp/include` is a **curated, vendored**
copy of just those headers, and `keystore/aosp/stub_{binder,utils}.cpp` are **ours** —
empty definitions carrying the right C++ ABI/SONAMEs so the device's real libraries
resolve them at runtime. Because it mixes a hand-picked vendoring with our own stubs and
is tightly coupled to that one interceptor, it lives next to the code that uses it rather
than under `third_party` (which is submodules only).
