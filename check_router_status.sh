#!/bin/bash

# Color codes for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}=== LLM Router Status Check ===${NC}"
echo

# Check if Docker is running
echo -e "${YELLOW}Checking Docker status:${NC}"
if docker info >/dev/null 2>&1; then
    echo -e "${GREEN}✓ Docker is running${NC}"
else
    echo -e "${RED}✗ Docker is not running. Please start Docker service.${NC}"
    exit 1
fi

# Check Docker Compose version
echo -e "\n${YELLOW}Checking Docker Compose version:${NC}"
COMPOSE_VERSION=$(docker compose version --short)
echo -e "${GREEN}✓ Docker Compose version: $COMPOSE_VERSION${NC}"

# Check running containers
echo -e "\n${YELLOW}Checking LLM Router containers:${NC}"
ROUTER_CONTROLLER=$(docker ps -q --filter "name=router-controller")
ROUTER_SERVER=$(docker ps -q --filter "name=router-server")

if [ -z "$ROUTER_CONTROLLER" ]; then
    echo -e "${RED}✗ Router Controller container is not running${NC}"
else
    echo -e "${GREEN}✓ Router Controller container is running${NC}"
    # Get status of the Router Controller service
    echo -e "\n${YELLOW}Router Controller service status:${NC}"
    CONTROLLER_STATUS=$(curl -s http://localhost:8085/health)
    if [ "$CONTROLLER_STATUS" == '{"status":"ok"}' ]; then
        echo -e "${GREEN}✓ Router Controller service is healthy${NC}"
    else
        echo -e "${RED}✗ Router Controller service is not responding properly: $CONTROLLER_STATUS${NC}"
    fi
fi

if [ -z "$ROUTER_SERVER" ]; then
    echo -e "${RED}✗ Router Server container is not running${NC}"
else
    echo -e "${GREEN}✓ Router Server container is running${NC}"
    # Check if Triton server is responding
    echo -e "\n${YELLOW}Triton Server status:${NC}"
    TRITON_STATUS=$(curl -s http://localhost:8010/v2/health/ready)
    if [ "$TRITON_STATUS" == '{"health":"ready"}' ]; then
        echo -e "${GREEN}✓ Triton Server is ready${NC}"
    else
        echo -e "${RED}✗ Triton Server is not responding properly: $TRITON_STATUS${NC}"
    fi
fi

# Check available models
echo -e "\n${YELLOW}Checking available Triton models:${NC}"
MODELS=$(curl -s http://localhost:8010/v2/models)
if [[ $MODELS == *"task_router_ensemble"* ]]; then
    echo -e "${GREEN}✓ Task router ensemble model is available${NC}"
else
    echo -e "${RED}✗ Task router ensemble model is not available${NC}"
fi

if [[ $MODELS == *"complexity_router_ensemble"* ]]; then
    echo -e "${GREEN}✓ Complexity router ensemble model is available${NC}"
else
    echo -e "${RED}✗ Complexity router ensemble model is not available${NC}"
fi

# Print instructions to run the test script
echo -e "\n${YELLOW}=== Next Steps ===${NC}"
echo -e "To test the routing with the updated models, run the test script:"
echo -e "${GREEN}python test_router_models.py${NC}"
echo

# Check if NVIDIA API key is set
if [ -z "$NVIDIA_API_KEY" ]; then
    echo -e "${RED}WARNING: NVIDIA_API_KEY environment variable is not set.${NC}"
    echo -e "Please set it before running the test script:"
    echo -e "${GREEN}export NVIDIA_API_KEY=your_api_key${NC}"
fi 