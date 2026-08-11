# Builds the mgc-app Android APK with cargo-apk2.
#
# cargo-apk2 reads the `[package.metadata.android]` table in
# `../Cargo.toml`, compiles any Java sources under `src/main/java`,
# packages the Rust cdylib and extra native libs from `src/main/jniLibs`,
# and produces a signed APK.
#
# Requires:
#   ANDROID_SDK_ROOT  — the SDK root containing build-tools/ and platforms/
#   cargo-apk2        — `cargo install cargo-apk2`
#
# This Makefile points ANDROID_NDK_ROOT at the same NDK revision that
# `.cargo/config.toml` uses for the aarch64-linux-android linker, and
# creates a debug keystore for the release profile if one doesn't exist.
#
# Usage:
#   make apk        # target/release/apk/mgcarpet-vr.apk
#   make install    # adb install -r the above
#   make run        # build, install, and launch on a connected device
#   make clean

WORKSPACE := ../../..
APK := $(WORKSPACE)/target/release/apk/mgcarpet-vr.apk
# Must match the linker path in .cargo/config.toml.
NDK_ROOT := /home/nathanaela/Android/Sdk/ndk/30.0.14904198
KEYSTORE := build/debug.keystore

# any installed platform >= minSdk works, so just take the newest.
PACKAGE_NAME := com.mgcarpet.vr
LIB_NAME := mgc_app
PLATFORM_API := 32
PLATFORM_JAR := $(lastword $(sort $(wildcard $(ANDROID_SDK_ROOT)/platforms/android-*/android.jar)))
BUILD_TOOLS := $(lastword $(sort $(wildcard $(ANDROID_SDK_ROOT)/build-tools/*)))
AAPT2 := $(BUILD_TOOLS)/aapt2
ZIPALIGN := $(BUILD_TOOLS)/zipalign
APKSIGNER := $(BUILD_TOOLS)/apksigner
STRIP := $(lastword $(sort $(wildcard $(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/*/bin/llvm-strip)))
SO := $(WORKSPACE)/target/aarch64-linux-android/release/lib$(LIB_NAME).so

BUILD := build
KEYSTORE := $(BUILD)/debug.keystore
APK := $(BUILD)/mgcarpet-vr.apk



.PHONY: apk install run clean

$(KEYSTORE):
	mkdir -p build
	keytool -genkeypair -v -keystore $(KEYSTORE) -storepass android \
		-alias mgcarpetvr -keyalg RSA -keysize 2048 -validity 10000 \
		-dname "CN=mgcarpet debug,O=mgcarpet,C=US" -keypass android

apk: $(KEYSTORE)
	cd $(WORKSPACE) && ANDROID_NDK_ROOT=$(NDK_ROOT) cargo apk2 build -p mgc-app --release --lib


so:
	cd $(WORKSPACE) && ANDROID_NDK_HOME=$(ANDROID_NDK_HOME) \
		cargo ndk -t arm64-v8a -P $(PLATFORM_API) build --release -p mgc-app

apk2: so $(KEYSTORE)
	mkdir -p $(BUILD)/lib/arm64-v8a $(BUILD)/repack
	$(STRIP) --strip-debug $(SO) -o $(BUILD)/lib/arm64-v8a/lib$(LIB_NAME).so
	$(AAPT2) link -o $(BUILD)/base.apk -I $(PLATFORM_JAR) --manifest AndroidManifest.xml
	rm -rf $(BUILD)/repack && mkdir -p $(BUILD)/repack/lib/arm64-v8a
	cd $(BUILD)/repack && unzip -qo ../base.apk
	cp $(BUILD)/../libopenxr_loader.so $(BUILD)/repack/lib/arm64-v8a/
	cp $(BUILD)/lib/arm64-v8a/lib$(LIB_NAME).so $(BUILD)/repack/lib/arm64-v8a/
	rm -f $(BUILD)/unsigned.apk
	cd $(BUILD)/repack && zip -qX -0 ../unsigned.apk lib/arm64-v8a/lib$(LIB_NAME).so
	cd $(BUILD)/repack && zip -qX -0 ../unsigned.apk lib/arm64-v8a/libopenxr_loader.so
	cd $(BUILD)/repack && zip -qX ../unsigned.apk AndroidManifest.xml resources.arsc
	$(ZIPALIGN) -f -p 4 $(BUILD)/unsigned.apk $(BUILD)/aligned.apk
	$(APKSIGNER) sign --ks $(KEYSTORE) --ks-pass pass:android \
		--ks-key-alias mgcarpetvr --out $(APK) $(BUILD)/aligned.apk
	@echo "signed APK: $(APK)"


install: apk
	adb install -r $(APK)

run:
	cd $(WORKSPACE) && ANDROID_NDK_ROOT=$(NDK_ROOT) cargo apk2 run -p mgc-app --release --lib

clean:
	rm -rf build
	cd $(WORKSPACE) && cargo clean -p mgc-app
