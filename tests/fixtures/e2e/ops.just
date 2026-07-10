set positional-arguments := true

# Run a recipe through a module namepath.
module-run value="module-default":
    @printf 'module:%s\n' "$1" >> "$JTV_E2E_LOG"
