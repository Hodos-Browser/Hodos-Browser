#!/bin/bash
# Test script to call acquireCertificate with localhost:3001 test server

curl -X POST http://localhost:3301/acquireCertificate \
  -H "Content-Type: application/json" \
  -d '{
    "acquisitionProtocol": 2,
    "certifierUrl": "http://localhost:3001",
    "type": "AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=",
    "fields": {
      "cool": "true"
    }
  }'


