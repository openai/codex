FROM rust:latest

WORKDIR /usr/num_cpus

COPY . .

RUN cargo build

CMD [ "cargo", "test", "--lib" ]
