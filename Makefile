build:
	docker build --progress=plain -t codex-cli .

codex:
	docker run -it --rm --user "$(id -u):$(id -g)" -v ./.cpanm:/root/.cpanm -v ~/.codex:/root/.codex -v ./img:/app codex-cli
