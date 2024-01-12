all: build copy

build:
	cargo build --release

copy:
	cp target/release/shake128 shake128

clean:
	cargo clean
	rm shake128
