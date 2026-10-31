APPLE   := apple
GEN     := $(APPLE)/generated
BUILD   := $(APPLE)/build.noindex
APP_DIR := $(BUILD)/Klippy.app
ASSETS  := $(APPLE)/Assets

# Sign with the local identity from `make dev-cert` when it exists, so the
# app keeps one designated requirement across rebuilds; otherwise ad hoc.
DEV_CERT := Klippy Development
SIGN_ID  := $(shell security find-identity -v -p codesigning 2>/dev/null | grep -q '"$(DEV_CERT)"' && echo "$(DEV_CERT)" || echo -)
CONFIG  ?= debug

# With only the Command Line Tools (no Xcode), swift test needs to be told where Swift Testing lives
DEV_DIR := $(shell xcode-select -p)
ifneq (,$(findstring CommandLineTools,$(DEV_DIR)))
TESTING_FW := $(DEV_DIR)/Library/Developer/Frameworks
SWIFT_TEST_FLAGS := -Xswiftc -F -Xswiftc $(TESTING_FW) -Xlinker -F -Xlinker $(TESTING_FW) \
	-Xlinker -rpath -Xlinker $(TESTING_FW) -Xlinker -rpath -Xlinker $(DEV_DIR)/Library/Developer/usr/lib
endif

ifeq ($(CONFIG),release)
RUST_PROFILE := release
CARGO_FLAGS  := --release
else
RUST_PROFILE := debug
CARGO_FLAGS  :=
endif

.PHONY: all test typegen package generate build icon dev-cert app run dmg open clean

all: app

# Rust core unit tests, then Swift shell tests against the real core
test: generate
	cargo test -p shared
	cd $(APPLE) && swift test $(SWIFT_TEST_FLAGS)

# Swift types for Event / ViewModel / Effect -> apple/generated/App
typegen:
	RUST_LOG=info cargo run -p shared --bin codegen --features codegen -- --output-dir $(GEN)

# Rust static library + BoltFFI Swift bindings -> apple/generated/Shared
# (boltffi pack apple needs full Xcode, so the package is assembled here instead)
package:
	rm -rf $(GEN)/Shared
	mkdir -p $(GEN)/Shared/Shared $(GEN)/Shared/SharedFFI/include $(GEN)/Shared/lib
	MACOSX_DEPLOYMENT_TARGET=14.0 cargo build -p shared --lib $(CARGO_FLAGS)
	cp target/$(RUST_PROFILE)/libshared.a $(GEN)/Shared/lib/
	cd shared && boltffi generate swift -o $(CURDIR)/$(GEN)/Shared/tmp
	mv $(GEN)/Shared/tmp/*.swift $(GEN)/Shared/Shared/
	mv $(GEN)/Shared/tmp/boltffi.h $(GEN)/Shared/SharedFFI/include/
	echo '// SwiftPM needs a source file in every C target' > $(GEN)/Shared/SharedFFI/shim.c
	rm -rf $(GEN)/Shared/tmp

generate: typegen package

# Build the SwiftUI shell with SwiftPM (no Xcode project needed)
build: generate
	cd $(APPLE) && swift build -c $(CONFIG)

# Regenerate the committed AppIcon.icns from AppIcon.svg (every size macOS wants)
icon:
	rm -rf $(BUILD)/AppIcon.iconset
	mkdir -p $(BUILD)/AppIcon.iconset
	swiftc -O $(ASSETS)/render-icon.swift -o $(BUILD)/render-icon
	for size in 16 32 128 256 512; do \
		$(BUILD)/render-icon $(ASSETS)/AppIcon.svg $(BUILD)/AppIcon.iconset/icon_$${size}x$${size}.png $$size; \
		$(BUILD)/render-icon $(ASSETS)/AppIcon.svg $(BUILD)/AppIcon.iconset/icon_$${size}x$${size}@2x.png $$((size * 2)); \
	done
	iconutil -c icns $(BUILD)/AppIcon.iconset -o $(ASSETS)/AppIcon.icns

# One-time: create the stable local signing identity (asks for your password)
dev-cert:
	./scripts/make-dev-cert.sh

# Wrap the executable in a .app bundle
app: build
	rm -rf $(APP_DIR)
	mkdir -p $(APP_DIR)/Contents/MacOS $(APP_DIR)/Contents/Resources
	cp $(APPLE)/Info.plist $(APP_DIR)/Contents/Info.plist
	cp $(ASSETS)/AppIcon.icns $(APP_DIR)/Contents/Resources/AppIcon.icns
	cp "$$(cd $(APPLE) && swift build -c $(CONFIG) --show-bin-path)/Klippy" $(APP_DIR)/Contents/MacOS/Klippy
	codesign --force --sign "$(SIGN_ID)" $(APP_DIR)
	@[ "$(SIGN_ID)" != "-" ] || echo "note: signed ad hoc; run 'make dev-cert' once to sign with a stable local certificate"

run: app
	open $(APP_DIR)

# Release disk image with the app and an Applications shortcut to drag it onto
VERSION := $(shell /usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' $(APPLE)/Info.plist)
DMG     := $(BUILD)/Klippy-$(VERSION).dmg

dmg:
	$(MAKE) app CONFIG=release
	rm -rf $(BUILD)/dmg $(DMG)
	mkdir -p $(BUILD)/dmg
	cp -R $(APP_DIR) $(BUILD)/dmg/
	ln -s /Applications $(BUILD)/dmg/Applications
	hdiutil create -volname "Klippy $(VERSION)" -srcfolder $(BUILD)/dmg -fs HFS+ -format UDZO -ov $(DMG)
	rm -rf $(BUILD)/dmg
	@echo "built $(DMG)"

# Edit the Swift shell in Xcode (opens Package.swift directly)
open:
	open $(APPLE)/Package.swift

clean:
	cargo clean
	rm -rf $(GEN) $(APPLE)/.build $(BUILD)
