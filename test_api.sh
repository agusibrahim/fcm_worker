#!/bin/bash
# FCM Receiver API Test Script
# Run this after starting the server with: cargo run

set -e

# Configuration
BASE_URL="${BASE_URL:-http://localhost:3000}"
API_KEY="${API_KEY:-test_key}"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counter
PASSED=0
FAILED=0

# Helper function
test_endpoint() {
    local method="$1"
    local endpoint="$2"
    local data="$3"
    local expected_status="$4"
    local description="$5"
    
    echo -n "Testing: $description... "
    
    if [ -n "$data" ]; then
        response=$(curl -s -w "\n%{http_code}" -X "$method" "$BASE_URL$endpoint" \
            -H "X-API-Key: $API_KEY" \
            -H "Content-Type: application/json" \
            -d "$data" 2>&1)
    else
        response=$(curl -s -w "\n%{http_code}" -X "$method" "$BASE_URL$endpoint" \
            -H "X-API-Key: $API_KEY" 2>&1)
    fi
    
    status_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | sed '$d')
    
    if [ "$status_code" = "$expected_status" ]; then
        echo -e "${GREEN}PASS${NC} (HTTP $status_code)"
        ((PASSED++))
        echo "$body" | head -c 200
        echo ""
    else
        echo -e "${RED}FAIL${NC} (Expected $expected_status, got $status_code)"
        ((FAILED++))
        echo "Response: $body"
    fi
    echo "---"
}

echo "========================================"
echo "FCM Receiver API Test Suite"
echo "Base URL: $BASE_URL"
echo "========================================"
echo ""

# Check if server is running
echo "Checking server status..."
health_response=$(curl -s "$BASE_URL/health" 2>&1 || echo "FAILED")
if echo "$health_response" | grep -q '"status":"ok"'; then
    echo -e "${GREEN}Server is running${NC}"
else
    echo -e "${RED}Server is not running!${NC}"
    echo "Please start the server with: cargo run"
    exit 1
fi
echo ""

# First, get API key from environment or prompt
if [ "$API_KEY" = "test_key" ]; then
    echo -e "${YELLOW}Note: Using default test_key. Set API_KEY env var to your actual key.${NC}"
    echo "Example: API_KEY=your_key ./test_api.sh"
    echo ""
fi

echo "========== Health Endpoints =========="
test_endpoint "GET" "/health" "" "200" "Health check (no auth)"

# Test with API key
test_endpoint "GET" "/api/stats" "" "200" "Stats endpoint"

echo ""
echo "========== Credential CRUD =========="

# Create credential
CRED_DATA='{
    "name": "Test FCM App",
    "api_key": "AIzaSyTestKey123",
    "app_id": "1:123456789:android:abc123",
    "project_id": "test-project",
    "webhook_url": "https://webhook.site/test",
    "topics": ["test-topic"]
}'
test_endpoint "POST" "/api/credentials" "$CRED_DATA" "200" "Create credential"

# List credentials
echo "Getting credential list..."
list_response=$(curl -s "$BASE_URL/api/credentials" -H "X-API-Key: $API_KEY")
echo "Response: $(echo "$list_response" | head -c 300)"
echo ""

# Extract first credential ID
CRED_ID=$(echo "$list_response" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -n "$CRED_ID" ]; then
    echo "Found credential ID: $CRED_ID"
    echo ""
    
    test_endpoint "GET" "/api/credentials/$CRED_ID" "" "200" "Get single credential"
    
    # Update credential
    UPDATE_DATA='{"name": "Updated FCM App", "webhook_url": "https://webhook.site/updated"}'
    test_endpoint "PUT" "/api/credentials/$CRED_ID" "$UPDATE_DATA" "200" "Update credential"
    
    # Worker control
    test_endpoint "POST" "/api/credentials/$CRED_ID/start" "" "200" "Start listener"
    test_endpoint "POST" "/api/credentials/$CRED_ID/stop" "" "200" "Stop listener"
    test_endpoint "POST" "/api/credentials/$CRED_ID/suspend" "" "200" "Suspend credential"
    test_endpoint "POST" "/api/credentials/$CRED_ID/unsuspend" "" "200" "Unsuspend credential"
    
    # Delete credential
    test_endpoint "DELETE" "/api/credentials/$CRED_ID" "" "200" "Delete credential"
else
    echo -e "${RED}Could not extract credential ID from response${NC}"
    ((FAILED++))
fi

echo ""
echo "========== Message Endpoints =========="
test_endpoint "GET" "/api/messages" "" "200" "List messages"

echo ""
echo "========== Auth Tests =========="
echo -n "Testing: Invalid API key... "
invalid_response=$(curl -s -w "\n%{http_code}" "$BASE_URL/api/credentials" \
    -H "X-API-Key: invalid_key" 2>&1)
invalid_status=$(echo "$invalid_response" | tail -n1)
if [ "$invalid_status" = "401" ]; then
    echo -e "${GREEN}PASS${NC} (correctly rejected)"
    ((PASSED++))
else
    echo -e "${RED}FAIL${NC} (Expected 401, got $invalid_status)"
    ((FAILED++))
fi

echo ""
echo "========================================"
echo "Test Results: ${GREEN}$PASSED passed${NC}, ${RED}$FAILED failed${NC}"
echo "========================================"

if [ $FAILED -gt 0 ]; then
    exit 1
fi
