#!/usr/bin/env bash

# Tool Builder Docker Launcher
# Easy one-command launch script for the complete Tool Builder system

set -euo pipefail

echo "🚀 Launching Codex Tool Builder System..."

# Check prerequisites
if ! command -v docker &> /dev/null; then
    echo "❌ Docker is required but not installed"
    echo "Please install Docker from: https://docs.docker.com/get-docker/"
    exit 1
fi

if ! command -v docker-compose &> /dev/null; then
    echo "❌ Docker Compose is required but not installed"
    echo "Please install Docker Compose from: https://docs.docker.com/compose/install/"
    exit 1
fi

# Check if .env file exists, create if not
if [ ! -f .env ]; then
    echo "📝 Creating environment configuration..."
    cat > .env << EOF
OPENAI_API_KEY=${OPENAI_API_KEY:-your-openai-api-key-here}
GITHUB_TOKEN=${GITHUB_TOKEN:-your-github-token-here}
NODE_ENV=production
DATABASE_URL=postgresql://postgres:postgres@tool-builder-db:5432/tool_builder
REDIS_URL=redis://tool-builder-redis:6379
EOF
    echo "⚠️  Please edit .env file with your API keys before running again"
    echo "   Required: OPENAI_API_KEY and GITHUB_TOKEN"
    exit 1
fi

# Source environment variables
source .env

# Validate required environment variables
if [[ "${OPENAI_API_KEY}" == "your-openai-api-key-here" ]]; then
    echo "❌ Please set your OPENAI_API_KEY in the .env file"
    exit 1
fi

if [[ "${GITHUB_TOKEN}" == "your-github-token-here" ]]; then
    echo "❌ Please set your GITHUB_TOKEN in the .env file"
    exit 1
fi

echo "✅ Environment configuration validated"

# Create necessary directories
mkdir -p generated-tools
mkdir -p services/nginx

# Create nginx configuration
cat > services/nginx/nginx.conf << 'EOF'
events {
    worker_connections 1024;
}

http {
    upstream ui {
        server tool-builder-ui:3000;
    }
    
    upstream api {
        server tool-builder-api:8000;
    }
    
    server {
        listen 80;
        
        # UI routes
        location / {
            proxy_pass http://ui;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }
        
        # API routes
        location /api {
            proxy_pass http://api;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }
        
        # WebSocket support
        location /ws {
            proxy_pass http://api;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "upgrade";
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }
    }
}
EOF

echo "🐳 Starting Docker containers..."

# Build and start services
docker-compose -f docker-compose.tool-builder.yml up --build -d

echo "⏳ Waiting for services to start..."
sleep 10

# Check if services are healthy
echo "🏥 Checking service health..."

# Check API health
if curl -f http://localhost:8000/health &>/dev/null; then
    echo "✅ API service is healthy"
else
    echo "⚠️  API service may still be starting..."
fi

# Check UI availability
if curl -f http://localhost:3000 &>/dev/null; then
    echo "✅ UI service is healthy"
else
    echo "⚠️  UI service may still be starting..."
fi

echo ""
echo "🎉 Tool Builder System is now running!"
echo ""
echo "🌐 Web UI:  http://localhost:3000"
echo "📡 API:     http://localhost:8000"
echo "📖 API Docs: http://localhost:8000/docs"
echo ""
echo "📊 System Status:"
echo "   Database: PostgreSQL on port 5432"
echo "   Cache:    Redis on port 6379"
echo "   Proxy:    Nginx on port 80"
echo ""
echo "🔧 Management Commands:"
echo "   View logs:    docker-compose -f docker-compose.tool-builder.yml logs -f"
echo "   Stop system:  docker-compose -f docker-compose.tool-builder.yml down"
echo "   Restart:      docker-compose -f docker-compose.tool-builder.yml restart"
echo ""

# Auto-open browser (macOS/Linux)
if command -v open &> /dev/null; then
    echo "🌐 Opening Tool Builder in your browser..."
    open http://localhost:3000
elif command -v xdg-open &> /dev/null; then
    echo "🌐 Opening Tool Builder in your browser..."
    xdg-open http://localhost:3000
else
    echo "💡 Open http://localhost:3000 in your browser to start using Tool Builder!"
fi

echo ""
echo "🚀 Happy tool building! Generate your first CLI tool in minutes, not hours!"