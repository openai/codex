"""
Tool Builder API - FastAPI backend for the Tool Builder Web UI
Provides REST endpoints for tool generation and management
"""

from fastapi import FastAPI, HTTPException, BackgroundTasks, WebSocket
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
from typing import Optional, List, Dict
import subprocess
import asyncio
import json
import os
import uuid
from datetime import datetime
import redis
import psycopg2
from psycopg2.extras import RealDictCursor

app = FastAPI(
    title="Tool Builder API",
    description="Autonomous CLI tool generation and management",
    version="1.0.0"
)

# CORS configuration
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000", "http://tool-builder-ui:3000"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Redis connection
redis_client = redis.Redis(
    host=os.getenv("REDIS_URL", "localhost").split("://")[1].split(":")[0],
    port=6379,
    decode_responses=True
)

# Database connection
def get_db():
    conn = psycopg2.connect(
        os.getenv("DATABASE_URL", "postgresql://postgres:postgres@localhost:5432/tool_builder"),
        cursor_factory=RealDictCursor
    )
    return conn

# Initialize database
def init_db():
    conn = get_db()
    cur = conn.cursor()
    cur.execute("""
        CREATE TABLE IF NOT EXISTS tools (
            id UUID PRIMARY KEY,
            name VARCHAR(255) UNIQUE NOT NULL,
            description TEXT,
            status VARCHAR(50),
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            github_url TEXT,
            codespace_url TEXT,
            design_spec TEXT,
            error_log TEXT
        )
    """)
    conn.commit()
    conn.close()

# Models
class ToolRequest(BaseModel):
    name: str
    description: str
    template: Optional[str] = "typescript"
    features: Optional[List[str]] = []

class ToolResponse(BaseModel):
    id: str
    name: str
    description: str
    status: str
    created_at: datetime
    github_url: Optional[str]
    codespace_url: Optional[str]

class ToolStatus(BaseModel):
    id: str
    status: str
    current_step: str
    progress: int
    logs: List[str]

# WebSocket manager for real-time updates
class ConnectionManager:
    def __init__(self):
        self.active_connections: Dict[str, WebSocket] = {}

    async def connect(self, websocket: WebSocket, client_id: str):
        await websocket.accept()
        self.active_connections[client_id] = websocket

    def disconnect(self, client_id: str):
        if client_id in self.active_connections:
            del self.active_connections[client_id]

    async def send_personal_message(self, message: str, client_id: str):
        if client_id in self.active_connections:
            await self.active_connections[client_id].send_text(message)

    async def broadcast(self, message: str):
        for connection in self.active_connections.values():
            await connection.send_text(message)

manager = ConnectionManager()

# Endpoints
@app.on_event("startup")
async def startup_event():
    init_db()

@app.get("/")
async def root():
    return {
        "message": "Tool Builder API",
        "version": "1.0.0",
        "endpoints": {
            "create_tool": "/tools/create",
            "list_tools": "/tools",
            "tool_status": "/tools/{tool_id}/status",
            "websocket": "/ws/{client_id}"
        }
    }

@app.post("/tools/create", response_model=ToolResponse)
async def create_tool(request: ToolRequest, background_tasks: BackgroundTasks):
    """Create a new CLI tool using the tool-builder.sh script"""
    
    # Generate unique ID
    tool_id = str(uuid.uuid4())
    
    # Store initial record in database
    conn = get_db()
    cur = conn.cursor()
    cur.execute("""
        INSERT INTO tools (id, name, description, status)
        VALUES (%s, %s, %s, %s)
    """, (tool_id, request.name, request.description, "initializing"))
    conn.commit()
    
    # Get the created record
    cur.execute("SELECT * FROM tools WHERE id = %s", (tool_id,))
    tool = cur.fetchone()
    conn.close()
    
    # Start background task for tool generation
    background_tasks.add_task(generate_tool_async, tool_id, request)
    
    return ToolResponse(**tool)

async def generate_tool_async(tool_id: str, request: ToolRequest):
    """Background task to generate tool using tool-builder.sh"""
    
    try:
        # Update status
        update_tool_status(tool_id, "running", "Starting tool generation...")
        
        # Prepare command
        cmd = [
            "/app/tools/tool-builder.sh",
            request.name,
            request.description
        ]
        
        # Run tool-builder.sh with real-time output capture
        process = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            env={**os.environ, "TOOL_ID": tool_id}
        )
        
        # Stream output to WebSocket
        step_mapping = {
            "[0/8]": ("validating", 5),
            "[1/8]": ("designing", 15),
            "[2/8]": ("scaffolding", 30),
            "[3/8]": ("installing", 45),
            "[4/8]": ("github", 60),
            "[5/8]": ("devcontainer", 70),
            "[6/8]": ("codespace", 85),
            "[7/8]": ("cleanup", 95),
            "[8/8]": ("complete", 100)
        }
        
        logs = []
        async for line in process.stdout:
            line_str = line.decode().strip()
            logs.append(line_str)
            
            # Detect step and update progress
            for step_marker, (step_name, progress) in step_mapping.items():
                if step_marker in line_str:
                    await update_and_broadcast(tool_id, step_name, progress, logs)
                    break
            
            # Store logs in Redis for retrieval
            redis_client.rpush(f"tool:{tool_id}:logs", line_str)
        
        # Wait for process completion
        await process.wait()
        
        if process.returncode == 0:
            # Extract GitHub URL from logs
            github_url = extract_github_url(logs)
            codespace_url = f"{github_url}/codespaces/new" if github_url else None
            
            # Update database with success
            conn = get_db()
            cur = conn.cursor()
            cur.execute("""
                UPDATE tools 
                SET status = 'completed',
                    github_url = %s,
                    codespace_url = %s,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = %s
            """, (github_url, codespace_url, tool_id))
            conn.commit()
            conn.close()
            
            await update_and_broadcast(tool_id, "completed", 100, logs)
        else:
            # Handle failure
            error_output = await process.stderr.read()
            error_message = error_output.decode()
            
            conn = get_db()
            cur = conn.cursor()
            cur.execute("""
                UPDATE tools 
                SET status = 'failed',
                    error_log = %s,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = %s
            """, (error_message, tool_id))
            conn.commit()
            conn.close()
            
            await update_and_broadcast(tool_id, "failed", 0, logs, error_message)
            
    except Exception as e:
        # Handle unexpected errors
        error_message = str(e)
        conn = get_db()
        cur = conn.cursor()
        cur.execute("""
            UPDATE tools 
            SET status = 'error',
                error_log = %s,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = %s
        """, (error_message, tool_id))
        conn.commit()
        conn.close()
        
        await update_and_broadcast(tool_id, "error", 0, [], error_message)

def update_tool_status(tool_id: str, status: str, message: str):
    """Update tool status in database"""
    conn = get_db()
    cur = conn.cursor()
    cur.execute("""
        UPDATE tools 
        SET status = %s, updated_at = CURRENT_TIMESTAMP
        WHERE id = %s
    """, (status, tool_id))
    conn.commit()
    conn.close()

async def update_and_broadcast(tool_id: str, step: str, progress: int, logs: List[str], error: str = None):
    """Update status and broadcast to WebSocket clients"""
    status = {
        "id": tool_id,
        "status": step,
        "current_step": step,
        "progress": progress,
        "logs": logs[-10:],  # Last 10 log lines
        "error": error
    }
    
    # Broadcast to all connected clients
    await manager.broadcast(json.dumps(status))

def extract_github_url(logs: List[str]) -> Optional[str]:
    """Extract GitHub repository URL from tool-builder logs"""
    for line in logs:
        if "github.com" in line and "/pull/new/" not in line:
            # Extract URL from various log formats
            import re
            match = re.search(r'https://github\.com/[\w-]+/[\w-]+', line)
            if match:
                return match.group()
    return None

@app.get("/tools", response_model=List[ToolResponse])
async def list_tools(skip: int = 0, limit: int = 20):
    """List all generated tools"""
    conn = get_db()
    cur = conn.cursor()
    cur.execute("""
        SELECT * FROM tools 
        ORDER BY created_at DESC 
        LIMIT %s OFFSET %s
    """, (limit, skip))
    tools = cur.fetchall()
    conn.close()
    
    return [ToolResponse(**tool) for tool in tools]

@app.get("/tools/{tool_id}", response_model=ToolResponse)
async def get_tool(tool_id: str):
    """Get details of a specific tool"""
    conn = get_db()
    cur = conn.cursor()
    cur.execute("SELECT * FROM tools WHERE id = %s", (tool_id,))
    tool = cur.fetchone()
    conn.close()
    
    if not tool:
        raise HTTPException(status_code=404, detail="Tool not found")
    
    return ToolResponse(**tool)

@app.get("/tools/{tool_id}/status", response_model=ToolStatus)
async def get_tool_status(tool_id: str):
    """Get current status and logs of tool generation"""
    conn = get_db()
    cur = conn.cursor()
    cur.execute("SELECT status FROM tools WHERE id = %s", (tool_id,))
    result = cur.fetchone()
    conn.close()
    
    if not result:
        raise HTTPException(status_code=404, detail="Tool not found")
    
    # Get logs from Redis
    logs = redis_client.lrange(f"tool:{tool_id}:logs", 0, -1)
    
    # Determine progress based on status
    progress_map = {
        "initializing": 0,
        "validating": 5,
        "designing": 15,
        "scaffolding": 30,
        "installing": 45,
        "github": 60,
        "devcontainer": 70,
        "codespace": 85,
        "cleanup": 95,
        "completed": 100,
        "failed": 0,
        "error": 0
    }
    
    status = result["status"]
    return ToolStatus(
        id=tool_id,
        status=status,
        current_step=status,
        progress=progress_map.get(status, 0),
        logs=logs
    )

@app.delete("/tools/{tool_id}")
async def delete_tool(tool_id: str):
    """Delete a tool record"""
    conn = get_db()
    cur = conn.cursor()
    cur.execute("DELETE FROM tools WHERE id = %s", (tool_id,))
    deleted = cur.rowcount
    conn.commit()
    conn.close()
    
    if deleted == 0:
        raise HTTPException(status_code=404, detail="Tool not found")
    
    # Clean up Redis logs
    redis_client.delete(f"tool:{tool_id}:logs")
    
    return {"message": "Tool deleted successfully"}

@app.websocket("/ws/{client_id}")
async def websocket_endpoint(websocket: WebSocket, client_id: str):
    """WebSocket endpoint for real-time updates"""
    await manager.connect(websocket, client_id)
    try:
        while True:
            # Keep connection alive
            data = await websocket.receive_text()
            # Echo back or handle commands
            await manager.send_personal_message(f"Echo: {data}", client_id)
    except Exception as e:
        print(f"WebSocket error: {e}")
    finally:
        manager.disconnect(client_id)

@app.get("/health")
async def health_check():
    """Health check endpoint"""
    try:
        # Check database connection
        conn = get_db()
        cur = conn.cursor()
        cur.execute("SELECT 1")
        conn.close()
        
        # Check Redis connection
        redis_client.ping()
        
        return {
            "status": "healthy",
            "services": {
                "database": "connected",
                "redis": "connected",
                "tool_builder": os.path.exists("/app/tools/tool-builder.sh")
            }
        }
    except Exception as e:
        return {
            "status": "unhealthy",
            "error": str(e)
        }

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)