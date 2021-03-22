FROM alpine:edge as build

# stuff we need to build the application
RUN apk update
RUN apk add --no-cache gcc pkgconfig libressl libressl-dev musl-dev mariadb-dev rust cargo

# create a cargo project
WORKDIR /cargo-build
RUN ash -c 'USER=root cargo new --bin monkey'
WORKDIR /cargo-build/monkey

# copy cargo details
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# compile libraries seperatly
RUN ash -c 'cargo build --release'
RUN rm src/*.rs

# copy sources
COPY ./src ./src

# actual build
RUN rm ./target/release/deps/monkey*
RUN ash -c 'cargo build --release'




FROM alpine:edge

# libssl
RUN apk update
RUN apk add --no-cache libressl

# copy built files from temporary container
COPY --from=build /cargo-build/monkey/target/release/monkey .

# copy config and various data files
COPY config.docker.toml ./config.toml
COPY ./data ./data

# command to run
CMD ["./monkey"]

