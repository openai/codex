# Docker and Docker Compose Support

This repository supports Docker and Docker Compose for easier development and deployment of the Codex CLI. This document provides comprehensive guidance for Docker usage, testing, and production deployment.

## Prerequisites

- Docker
- Docker Compose

Note: The Docker container comes pre-configured with Node.js 22, so you don't need to install Node.js separately on your host system.

## Getting Started with Docker Compose

1. Build and start the container:

```bash
docker-compose build
docker-compose up -d
```

2. Enter the running container:

```bash
docker-compose exec codex-cli bash
```

3. Inside the container, you can use Codex CLI:

```bash
codex --help
```

## Testing the Docker Setup

We provide an automation-friendly testing script (`test_docker.sh`) to verify that the Docker setup works correctly with Node.js 22:

### Basic Usage

```bash
# Run basic test (builds, verifies, and stops container)
./test_docker.sh

# Keep container running after test
./test_docker.sh --keep

# Run quietly for CI environments
./test_docker.sh --quiet

# Show Node.js version information
./test_docker.sh --node-info
```

### What the Test Script Does

1. Checks if Docker Compose is installed
2. Builds the Docker container with Node.js 22
3. Starts the container in detached mode
4. Tests if Codex CLI works inside the container
5. Shows success or failure message
6. Stops the container (unless --keep is specified)

### Troubleshooting Docker Issues

If you encounter problems with the Docker setup:

1. Check Docker logs: `docker-compose logs`
2. Verify Node.js version: `docker-compose exec codex-cli node --version`
3. Check if Codex is properly installed:
   ```bash
   docker-compose exec codex-cli bash -c "ls -la /usr/local/share/npm-global/bin/"
   ```

### CI/CD Integration

The test script is designed for automated environments:

```yaml
# Example GitHub Actions workflow step
- name: Test Docker Setup
  run: ./test_docker.sh --quiet
```

## Using Docker Without Docker Compose

If you prefer to use Docker directly:

1. Build the Docker image:

```bash
cd codex-cli
./scripts/build_container.sh
```

2. Run a container with your desired working directory:

```bash
./codex-cli/scripts/run_in_container.sh --work_dir /path/to/your/project "your command"
```

## Security Features

The Docker container includes several security features:

- Network firewall that limits outbound connections to only approved domains
- Runs as a non-root user
- Has the necessary capabilities for network operations

## Environment Variables

- `TZ`: Sets the timezone in the container
- `CODEX_UNSAFE_ALLOW_NO_SANDBOX`: Set to 1 to allow running without sandboxing
- `OPENAI_API_KEY`: Your OpenAI API key for authentication
- `OPENAI_ALLOWED_DOMAINS`: Comma-separated list of domains allowed for outbound connections

## Volume Mounts

The Docker Compose configuration mounts your local repository to `/workspace` in the container.

## Production Deployment

For production environments, we provide a more robust deployment solution:

### Using the Production Deployment Script

We offer a dedicated production deployment script that handles error logging, container management, and verification:

```bash
# Make the script executable
chmod +x ./scripts/deploy_production.sh

# Run the deployment script
./scripts/deploy_production.sh
```

### Manual Production Deployment

If you prefer to deploy manually:

1. Build and start the production container:

```bash
docker-compose -f docker-compose.yml -f docker-compose.production.yml build
docker-compose -f docker-compose.yml -f docker-compose.production.yml up -d
```

2. Verify the deployment:

```bash
docker-compose -f docker-compose.yml -f docker-compose.production.yml exec codex-cli bash -c "codex --version"
```

### Production Configuration

The production configuration (`docker-compose.production.yml`) includes:

- Resource limits for CPU and memory
- Automatic container restart policy
- Health checks to monitor container status
- Improved security settings
- Separate volume mounts for data and logs

### Monitoring and Maintenance

- Check container logs:

  ```bash
  docker logs codex-production
  ```

- Enter the running container:

  ```bash
  docker exec -it codex-production bash
  ```

- Stop the production container:

  ```bash
  docker-compose -f docker-compose.yml -f docker-compose.production.yml down
  ```

- Update to a newer version:
  ```bash
  git pull
  ./scripts/deploy_production.sh
  ```
