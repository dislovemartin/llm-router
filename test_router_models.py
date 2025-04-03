#!/usr/bin/env python3
import requests
import json
import os
import time

# Configuration
ROUTER_URL = "http://localhost:8085/v1/chat/completions"
TASKS = [
    {"name": "Chatbot", "prompt": "Hello! How are you today? Tell me about yourself."},
    {"name": "Code Generation", "prompt": "Write a Python function to sort a list of dictionaries by a specific key."},
    {"name": "Text Generation", "prompt": "Write a short story about a robot who becomes self-aware."},
    {"name": "Question Answering", "prompt": "What is the capital of France and why is it historically significant?"},
    {"name": "Summarization", "prompt": "Summarize the key benefits of using containerization for application deployment."},
    {"name": "Classification", "prompt": "Categorize this text into a genre: 'The spaceship landed on the alien planet as the crew prepared for first contact.'"},
    {"name": "Extraction", "prompt": "Extract the names, dates, and locations from this text: 'John Smith met with Sarah Johnson on March 15, 2023 in New York City to discuss the new project launch in Tokyo scheduled for December 5.'"}
]

def test_task_router():
    """Test routing to different models based on task classification"""
    print("\n=== Testing Task Router ===\n")
    
    for task in TASKS:
        print(f"\nTesting task: {task['name']}")
        print(f"Prompt: {task['prompt']}")
        
        # Create the request payload
        payload = {
            "model": "",
            "messages": [
                {
                    "role": "user",
                    "content": task['prompt']
                }
            ],
            "max_tokens": 100,
            "stream": False,
            "nim-llm-router": {
                "policy": "task_router",
                "routing_strategy": "triton",
                "model": ""
            }
        }
        
        try:
            # Send the request
            response = requests.post(ROUTER_URL, json=payload)
            
            # Check the response
            if response.status_code == 200:
                result = response.json()
                model_used = result.get("model", "Unknown")
                print(f"✅ Success! Routed to model: {model_used}")
                first_tokens = result.get("choices", [{}])[0].get("message", {}).get("content", "")[:100]
                print(f"Response preview: {first_tokens}...")
            else:
                print(f"❌ Failed with status code: {response.status_code}")
                print(f"Error: {response.text}")
        except Exception as e:
            print(f"❌ Exception occurred: {str(e)}")
        
        # Sleep briefly to avoid rate limiting
        time.sleep(1)

def test_agentic_router():
    """Test routing using the agentic router"""
    print("\n=== Testing Agentic Router ===\n")
    
    for task in TASKS[:3]:  # Testing just a subset for brevity
        print(f"\nTesting task with agentic router: {task['name']}")
        print(f"Prompt: {task['prompt']}")
        
        # Create the request payload
        payload = {
            "model": "",
            "messages": [
                {
                    "role": "user",
                    "content": task['prompt']
                }
            ],
            "max_tokens": 100,
            "stream": False,
            "nim-llm-router": {
                "policy": "agentic_router",
                "routing_strategy": "triton",
                "model": ""
            }
        }
        
        try:
            # Send the request
            response = requests.post(ROUTER_URL, json=payload)
            
            # Check the response
            if response.status_code == 200:
                result = response.json()
                model_used = result.get("model", "Unknown")
                print(f"✅ Success! Agentic router selected model: {model_used}")
                first_tokens = result.get("choices", [{}])[0].get("message", {}).get("content", "")[:100]
                print(f"Response preview: {first_tokens}...")
            else:
                print(f"❌ Failed with status code: {response.status_code}")
                print(f"Error: {response.text}")
        except Exception as e:
            print(f"❌ Exception occurred: {str(e)}")
        
        # Sleep briefly to avoid rate limiting
        time.sleep(1)

def test_manual_routing():
    """Test manual routing to specific models"""
    print("\n=== Testing Manual Routing ===\n")
    
    # Models to test manual routing
    models = [
        "nvidia/llama-3.3-nemotron-super-49b-v1",
        "nvidia/nv-embedqa-mistral-7b-v2",
        "meta/llama-3.1-8b-instruct"
    ]
    
    for model in models:
        print(f"\nTesting manual routing to: {model}")
        
        # Create the request payload
        payload = {
            "model": "",
            "messages": [
                {
                    "role": "user",
                    "content": "Hello, this is a test for manual routing. Please identify which model you are."
                }
            ],
            "max_tokens": 100,
            "stream": False,
            "nim-llm-router": {
                "policy": "task_router",
                "routing_strategy": "manual",
                "model": model
            }
        }
        
        try:
            # Send the request
            response = requests.post(ROUTER_URL, json=payload)
            
            # Check the response
            if response.status_code == 200:
                result = response.json()
                model_used = result.get("model", "Unknown")
                print(f"✅ Success! Manually routed to model: {model_used}")
                first_tokens = result.get("choices", [{}])[0].get("message", {}).get("content", "")[:100]
                print(f"Response preview: {first_tokens}...")
            else:
                print(f"❌ Failed with status code: {response.status_code}")
                print(f"Error: {response.text}")
        except Exception as e:
            print(f"❌ Exception occurred: {str(e)}")
        
        # Sleep briefly to avoid rate limiting
        time.sleep(1)

if __name__ == "__main__":
    print("======================================")
    print("  LLM ROUTER MODEL TESTING SCRIPT")
    print("======================================")
    print("Testing routing to updated NVIDIA NIM API models")
    print("--------------------------------------")
    
    # Run all tests
    test_task_router()
    test_agentic_router()
    test_manual_routing()
    
    print("\n======================================")
    print("  TESTING COMPLETE")
    print("======================================") 