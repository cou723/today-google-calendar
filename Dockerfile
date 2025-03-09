# Raspberry Pi Zero 2用のクロスコンパイル環境
FROM rust:latest

WORKDIR /usr/src/myapp

# 複数アーキテクチャのサポートを追加
RUN dpkg --add-architecture arm64
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    binutils-arm-linux-gnueabihf \
    gcc-arm-linux-gnueabihf \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    # libssl-dev \
    libssl-dev:arm64 \
    libudev-dev:arm64 \
    libdbus-1-dev:arm64 \
    && rm -rf /var/lib/apt/lists/*

# Rustのクロスコンパイルターゲットを追加
RUN rustup target add aarch64-unknown-linux-gnu

# OpenSSLの設定
ENV OPENSSL_DIR=/usr/lib/aarch64-linux-gnu
ENV OPENSSL_INCLUDE_DIR=/usr/include
ENV OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu
ENV PKG_CONFIG_ALLOW_CROSS=1
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

# プロジェクトファイルをコピー
COPY . .
