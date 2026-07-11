# Prove that recursively discovered *.just files are first-class targets.
named-file:
    @printf 'named-file:%s\n' "$PWD" >> "$JTV_E2E_LOG"
