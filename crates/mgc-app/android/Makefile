# Packages the mgc-vr .so into a signed, sideloadable APK for Meta Quest 3.
#
# No Gradle: this project is a Cargo workspace everywhere else, and the
# manifest carries no resources (@string/@drawable etc.) — a direct
# aapt2 + zipalign + apksigner pipeline is the whole job, matching this
# repo's other non-Cargo build step (tools/mc2-genlevel/Makefile).
#
# Requires:
#   ANDROID_NDK_HOME  — an NDK install (e.g. .../Sdk/ndk/<version>)
#   ANDROID_SDK_ROOT  — the SDK root containing build-tools/ and platforms/
#   cargo-ndk         — `cargo install cargo-ndk`
#
# Usage:
#   make apk        # crates/mgc-vr/android/build/mgcarpet-vr.apk
#   make install     # adb install -r the above
#   make clean

PACKAGE_NAME := com.mgcarpet.vr
LIB_NAME := mgc_app
PLATFORM_API := 32
# NativeActivity + no Java/Kotlin code — minSdkVersion in the manifest
# is 26 (AAudio), but aapt2 needs a `-I android.jar` to link against;
# any installed platform >= minSdk works, so just take the newest.
PLATFORM_JAR := $(lastword $(sort $(wildcard $(ANDROID_SDK_ROOT)/platforms/android-*/android.jar)))
BUILD_TOOLS := $(lastword $(sort $(wildcard $(ANDROID_SDK_ROOT)/build-tools/*)))
AAPT2 := $(BUILD_TOOLS)/aapt2
ZIPALIGN := $(BUILD_TOOLS)/zipalign
APKSIGNER := $(BUILD_TOOLS)/apksigner
STRIP := $(lastword $(sort $(wildcard $(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/*/bin/llvm-strip)))

WORKSPACE := ../../..
SO := $(WORKSPACE)/target/aarch64-linux-android/release/lib$(LIB_NAME).so

BUILD := build
KEYSTORE := $(BUILD)/debug.keystore
APK := $(BUILD)/mgcarpet-vr.apk

.PHONY: so apk install clean

so:
	cd $(WORKSPACE) && ANDROID_NDK_HOME=$(ANDROID_NDK_HOME) \
		cargo ndk -t arm64-v8a -P $(PLATFORM_API) build --release -p mgc-app

$(KEYSTORE):
	mkdir -p $(BUILD)
	keytool -genkeypair -v -keystore $(KEYSTORE) -storepass android \
		-alias mgcarpetvr -keyalg RSA -keysize 2048 -validity 10000 \
		-dname "CN=mgcarpet debug,O=mgcarpet,C=US"

apk: so $(KEYSTORE)
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

clean:
	rm -rf $(BUILD)
