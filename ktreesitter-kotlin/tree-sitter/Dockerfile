FROM rust:1.76-buster

WORKDIR /app

RUN apt-get update
RUN apt-get install -y nodejs

COPY . .

CMD cargo test --all-features
