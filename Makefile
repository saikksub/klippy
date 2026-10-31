APPLE   := apple
GEN     := $(APPLE)/generated
APP_DIR := $(APPLE)/build/Klippy.app
ASSETS  := $(APPLE)/Assets
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

.PHONY: all test typegen package generate build icon app run open clean

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
	rm -rf $(APPLE)/build/AppIcon.iconset
	mkdir -p $(APPLE)/build/AppIcon.iconset
	swiftc -O $(ASSETS)/render-icon.swift -o $(APPLE)/build/render-icon
	for size in 16 32 128 256 512; do \
		$(APPLE)/build/render-icon $(ASSETS)/AppIcon.svg $(APPLE)/build/AppIcon.iconset/icon_$${size}x$${size}.png $$size; \
		$(APPLE)/build/render-icon $(ASSETS)/AppIcon.svg $(APPLE)/build/AppIcon.iconset/icon_$${size}x$${size}@2x.png $$((size * 2)); \
	done
	iconutil -c icns $(APPLE)/build/AppIcon.iconset -o $(ASSETS)/AppIcon.icns

# Wrap the executable in a .app bundle
app: build
	rm -rf $(APP_DIR)
	mkdir -p $(APP_DIR)/Contents/MacOS $(APP_DIR)/Contents/Resources
	cp $(APPLE)/Info.plist $(APP_DIR)/Contents/Info.plist
	cp $(ASSETS)/AppIcon.icns $(APP_DIR)/Contents/Resources/AppIcon.icns
	cp "$$(cd $(APPLE) && swift build -c $(CONFIG) --show-bin-path)/Klippy" $(APP_DIR)/Contents/MacOS/Klippy
	codesign --force --sign - $(APP_DIR)

run: app
	open $(APP_DIR)

# Edit the Swift shell in Xcode (opens Package.swift directly)
open:
	open $(APPLE)/Package.swift

clean:
	cargo clean
	rm -rf $(GEN) $(APPLE)/.build $(APPLE)/build
