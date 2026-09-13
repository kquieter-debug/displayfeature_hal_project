# DisplayFeature HAL Compatibility Project

Standalone Rust Android DisplayFeature HAL for the TB375FC HyperOS4 port.

## Layout

- rust/: Rust implementation using explicit libbinder_ndk FFI.
- aidl/: the single shared Stable AIDL contract.
- rust/: carries its own init/VINTF files and can be built independently.

The Rust implementation exposes the Xiaomi Stable AIDL service instance and is the only implementation in this project.

## Build

For the Rust implementation, use the Android platform rust_binary module in rust/Android.bp. Standalone Cargo linking requires the target device compatible libbinder_ndk.so and liblog.so; the checked-in device_lib64/ files are ABI references, not production system libraries.

The Rust service implements the HyperOS Stable AIDL wire contract and maps verified display features to the MTK PQ AIDL backend.

No commit or push is performed by the build workflow.
