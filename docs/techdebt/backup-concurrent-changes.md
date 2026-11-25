# Backup assumes nobody else is changing the folders

Restore checks that the target is an empty real folder, and create checks each file is still the one it scanned.
Someone who can change the parent folder at the same moment could still swap it for a link between the check and
the write. Closing this needs folder handle based operations. On Windows the swap check cannot compare file ids
without unsafe code.
