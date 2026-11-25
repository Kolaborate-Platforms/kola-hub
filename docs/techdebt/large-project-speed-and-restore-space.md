# Slow file lists and no free space check on restore

Listing a folder registers each file in its own database write, so thousands of files stall the list. Register
them in one transaction. Restore writes up to 50 GiB without first comparing that with the free disk space.
