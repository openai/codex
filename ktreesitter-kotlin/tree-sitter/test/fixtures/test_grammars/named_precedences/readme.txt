This grammar uses named precedences, which have a partial order specified via the grammar's `precedences` field. Named
precedences allow certain conflicts to be resolved statically without accidentally resolving *other* conflicts, which
are intended to be resolved dynamically.
