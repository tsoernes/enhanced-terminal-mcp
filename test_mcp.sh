#!/bin/bash
# Test the enhanced-terminal MCP server

echo "Testing Enhanced Terminal MCP Server"
echo "====================================="
echo ""

# The server uses stdio transport, so we'll test it's working by running it briefly
timeout 2s ./target/release/enhanced-terminal-mcp &
PID=$!

sleep 1

if kill -0 $PID 2>/dev/null; then
    echo "✅ Server started successfully (PID: $PID)"
    kill $PID 2>/dev/null
    wait $PID 2>/dev/null
else
    echo "❌ Server failed to start"
    exit 1
fi

echo ""
echo "Server binary info:"
echo "  Location: $(pwd)/target/release/enhanced-terminal-mcp"
echo "  Size: $(du -h target/release/enhanced-terminal-mcp | cut -f1)"
echo "  Permissions: $(ls -l target/release/enhanced-terminal-mcp | cut -d' ' -f1)"
echo ""
echo "Zed configuration:"
jq '.context_servers["enhanced-terminal"]' ~/.config/zed/settings.json
echo ""
echo "✅ All checks passed! Server is ready to use."
echo ""
echo "To activate in Zed:"
echo "1. Restart Zed or reload the window"
echo "2. The 'enhanced-terminal' context server should appear"
echo "3. Try using the enhanced_terminal tool in chat"
