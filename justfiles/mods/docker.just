# Docker module for container operations

# Build docker image
build image="app" tag="latest":
    @echo "Building Docker image: {{image}}:{{tag}}"
    @echo "docker build -t {{image}}:{{tag}} ."

# Push image to registry
push image="app" registry="docker.io":
    @echo "Pushing {{image}} to {{registry}}"
    @echo "docker push {{registry}}/{{image}}"

# Run container
run image="app" port="8080":
    @echo "Running container from {{image}} on port {{port}}"
    @echo "docker run -p {{port}}:{{port}} {{image}}"

# Clean up containers and images
clean:
    @echo "Cleaning up Docker resources"
    @echo "docker system prune -f"