from fastmcp import FastMCP

import sys

mcp = FastMCP("Demo")

print("server starting up...", file=sys.stderr)


@mcp.tool()
def add(a: int, b: int) -> int:
    """Add two numbers"""
    return a + b


# Add a dynamic greeting resource
@mcp.resource("greeting://{name}")
def get_greeting(name: str) -> str:
    """Get a personalized greeting"""
    return f"Hello, {name}!"



if __name__ == "__main__":
    mcp.run()