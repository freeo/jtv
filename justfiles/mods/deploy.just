# Deployment module

# Deploy to staging
staging branch="develop" dry_run="false":
    @echo "Deploying to staging from {{branch}}"
    @echo "Dry run: {{dry_run}}"
    @echo "kubectl apply -f k8s/staging/"

# Deploy to production
production version tag="stable":
    @echo "⚠️  Production deployment!"
    @echo "Deploying version: {{version}}"
    @echo "Tag: {{tag}}"
    @echo "kubectl apply -f k8s/production/"

# Rollback deployment
rollback env="staging" steps="1":
    @echo "Rolling back {{env}} by {{steps}} steps"
    @echo "kubectl rollout undo deployment/app -n {{env}}"

# Check deployment status
status env="staging":
    @echo "Checking {{env}} deployment status"
    @echo "kubectl rollout status deployment/app -n {{env}}"