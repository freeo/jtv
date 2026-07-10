create-new-cluster args="dev":
  echo "durr"

create-new-cluster args="dev":

# Show available commands
overview:
    @echo "🚀 Hetzner Crossplane Platform Commands"
    @echo ""
    @echo "Platform Admin Commands:"
    @echo "  just install-platform     - Install the complete platform"
    @echo "  just status-platform      - Check platform health"
    @echo "  just cleanup-platform     - Remove all platform components"
    @echo ""
    @echo "Team Lead Commands:"
    @echo "  just create-cluster NAME [SIZE] [REGION]  - Create a new cluster"
    @echo "  just delete-cluster NAME                  - Delete a cluster"
    @echo "  just get-cluster-ip NAME                  - Get cluster IP address"
    @echo "  just get-kubeconfig NAME                  - Retrieve cluster kubeconfig"
    @echo "  just cluster-status NAME                  - Show detailed cluster status"
    @echo ""
    @echo "Monitoring Commands:"
    @echo "  just watch-clusters       - Watch all cluster states"
    @echo "  just list-clusters        - List all clusters"
    @echo ""
    @echo "Example Usage:"
    @echo "  just create-cluster my-app small fsn1"
    @echo "  just get-cluster-ip my-app"
