#!/bin/bash

set -exuo pipefail

CONTAINER_ID=$(docker compose ps -q -a test)

docker container cp ${CONTAINER_ID}:/workspace/cobertura.xml ./
