#!/bin/sh

BUILDKIT_PROGRESS=plain DOCKER_BUILDKIT=1 docker build . -f Dockerfile -t janus_manager
