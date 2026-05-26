# ══════════════════════════════════════════════════════════════════════════════
# rustandroid — Hello World Android app with iced (Rust)
#
# Usage:
#   make setup      — install all dependencies (first time)
#   make build      — compile and package the debug APK
#   make release    — compile and package a release APK
#   make install    — push APK to connected device via adb
#   make run        — build + install + launch on device
#   make clean      — remove build artifacts
# ══════════════════════════════════════════════════════════════════════════════

ANDROID_HOME     ?= $(HOME)/android-sdk
ANDROID_NDK_ROOT ?= $(ANDROID_HOME)/ndk/25.2.9519653
SDKMAN_INIT      := $(HOME)/.sdkman/bin/sdkman-init.sh

JAVA_VERSION := 17.0.13-tem
NDK_VERSION  := 25.2.9519653
API_LEVEL    := 33

ANDROID_TARGETS := aarch64-linux-android armv7-linux-androideabi \
                   x86_64-linux-android i686-linux-android

APK_DEBUG   := target/debug/apk/rustandroid.apk
APK_RELEASE := target/release/apk/rustandroid.apk

PACKAGE     := com.example.rustandroid
ACTIVITY    := android.app.NativeActivity

# ── Export env vars used by every recipe ─────────────────────────────────────
export ANDROID_HOME
export ANDROID_NDK_ROOT
export PATH := $(ANDROID_HOME)/cmdline-tools/latest/bin:$(ANDROID_HOME)/platform-tools:$(PATH)

# Source SDKMAN so the Java 17 selected by `make setup` is on PATH
JAVA_HOME   := $(HOME)/.sdkman/candidates/java/current
export JAVA_HOME
export PATH := $(JAVA_HOME)/bin:$(PATH)

# ══════════════════════════════════════════════════════════════════════════════
.PHONY: setup setup-java setup-android-sdk setup-rust build release install run clean help

## Default target
all: build

# ── SETUP ─────────────────────────────────────────────────────────────────────

## Install every dependency needed to build the APK.
setup: setup-java setup-android-sdk setup-rust
	@echo ""
	@echo "✅  Setup complete. Run 'make build' to compile the APK."

## Install Java 17 via SDKMAN (required by Android SDK tools).
setup-java:
	@echo "── Java 17 ──────────────────────────────────────────────────────────"
	@if [ ! -f "$(SDKMAN_INIT)" ]; then \
	  echo "Installing SDKMAN…"; \
	  curl -s "https://get.sdkman.io" | bash; \
	fi
	@. $(SDKMAN_INIT) && \
	  if ! sdk list java 2>/dev/null | grep -q "$(JAVA_VERSION).*installed"; then \
	    echo "Installing Java $(JAVA_VERSION)…"; \
	    sdk install java $(JAVA_VERSION); \
	  else \
	    echo "Java $(JAVA_VERSION) already installed."; \
	  fi
	@echo "Java OK ✓"

## Install Android SDK, build-tools, platform API $(API_LEVEL), and NDK $(NDK_VERSION).
setup-android-sdk:
	@echo "── Android SDK ──────────────────────────────────────────────────────"
	@# Download command-line tools if not present
	@if [ ! -f "$(ANDROID_HOME)/cmdline-tools/latest/bin/sdkmanager" ]; then \
	  echo "Downloading Android command-line tools…"; \
	  mkdir -p $(ANDROID_HOME)/cmdline-tools; \
	  wget -q --show-progress \
	    "https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip" \
	    -O /tmp/cmdline-tools.zip; \
	  unzip -q /tmp/cmdline-tools.zip -d $(ANDROID_HOME)/cmdline-tools/; \
	  mv $(ANDROID_HOME)/cmdline-tools/cmdline-tools $(ANDROID_HOME)/cmdline-tools/latest; \
	  rm /tmp/cmdline-tools.zip; \
	fi
	@# Accept licenses and install components
	@. $(SDKMAN_INIT) 2>/dev/null; \
	  yes | sdkmanager --licenses > /dev/null 2>&1 || true; \
	  sdkmanager \
	    "platform-tools" \
	    "build-tools;34.0.0" \
	    "platforms;android-$(API_LEVEL)" \
	    "ndk;$(NDK_VERSION)"
	@echo "Android SDK OK ✓"

## Add Rust Android targets and install cargo-ndk / cargo-apk.
setup-rust:
	@echo "── Rust Android targets ─────────────────────────────────────────────"
	@for t in $(ANDROID_TARGETS); do \
	  rustup target add $$t 2>&1 | grep -v "^info: component.*already installed" || true; \
	done
	@echo "── cargo-apk ────────────────────────────────────────────────────────"
	@if ! cargo apk version > /dev/null 2>&1; then \
	  cargo install cargo-apk; \
	fi
	@echo "Rust toolchain OK ✓"

# ── BUILD ──────────────────────────────────────────────────────────────────────

## Compile debug APK (fast, not optimised).
build:
	@echo "── Building debug APK ───────────────────────────────────────────────"
	cargo apk build; \
	  CODE=$$?; \
	  if [ -f "$(APK_DEBUG)" ]; then \
	    echo ""; \
	    echo "✅  APK built: $(APK_DEBUG)"; \
	    ls -lh $(APK_DEBUG); \
	  else \
	    exit $$CODE; \
	  fi

## Compile release APK (optimised, smaller).
release:
	@echo "── Building release APK ─────────────────────────────────────────────"
	cargo apk build --release; \
	  CODE=$$?; \
	  if [ -f "$(APK_RELEASE)" ]; then \
	    echo ""; \
	    echo "✅  APK built: $(APK_RELEASE)"; \
	    ls -lh $(APK_RELEASE); \
	  else \
	    exit $$CODE; \
	  fi

# ── DEVICE ────────────────────────────────────────────────────────────────────

## Push the debug APK to a connected Android device.
install: build
	@echo "── Installing on device ─────────────────────────────────────────────"
	adb install -r $(APK_DEBUG)

## Build, install, and launch the app on a connected device.
run: install
	@echo "── Launching app ────────────────────────────────────────────────────"
	adb shell am start -n $(PACKAGE)/$(ACTIVITY)
	adb logcat -s RustStdoutStderr:D | head -50

# ── CLEAN ─────────────────────────────────────────────────────────────────────

## Remove Cargo build artifacts (keeps installed SDK/NDK).
clean:
	cargo clean

# ── HELP ──────────────────────────────────────────────────────────────────────
help:
	@echo ""
	@echo "  make setup     — install Java 17, Android SDK/NDK, Rust targets"
	@echo "  make build     — compile debug APK  →  $(APK_DEBUG)"
	@echo "  make release   — compile release APK"
	@echo "  make install   — push debug APK to device (needs adb)"
	@echo "  make run       — build + install + launch on device"
	@echo "  make clean     — remove build artifacts"
	@echo ""
