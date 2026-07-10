# Testing module for test operations

# Run unit tests
unit verbose="false":
    @echo "Running unit tests (verbose: {{verbose}})"
    @echo "npm test:unit"

# Run integration tests
integration env="test" timeout="30":
    @echo "Running integration tests"
    @echo "Environment: {{env}}"
    @echo "Timeout: {{timeout}}s"
    @echo "npm test:integration"

# Run all tests
all: unit integration
    @echo "All tests completed"

# Coverage report
coverage format="html":
    @echo "Generating coverage report in {{format}} format"
    @echo "npm test:coverage -- --format={{format}}"