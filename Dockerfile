FROM node:22-slim

# Set working directory
WORKDIR /app

# Copy entire project
COPY . .
# Install dependencies for building whisper.cpp
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential git cmake libblas3 liblapack3 \
    && rm -rf /var/lib/apt/lists/*
# Clone and build whisper.cpp for transcription
RUN git clone --depth 1 https://github.com/ggerganov/whisper.cpp.git /app/whisper.cpp \
    && cd /app/whisper.cpp \
    && make

# Ensure start script is executable
RUN chmod +x ./start.sh

# Expose server and frontend ports
EXPOSE 3000 5173

# Entry point to start both server and interface
CMD ["./start.sh"]