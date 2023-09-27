#!/bin/sh

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

BUILDKIT_PROGRESS=plain DOCKER_BUILDKIT=1 docker build "${SCRIPT_DIR}" -f "${SCRIPT_DIR}/Dockerfile" -t janus_manager
