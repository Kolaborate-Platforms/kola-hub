# Restoring a folder from Recently deleted can race with a new file

A restored file is placed with a hard link, so it can never replace an existing file. A folder is placed
with a check then a rename, so a file created in the tiny gap could be replaced on some systems. Files are
the only thing the tools put in the trash today. If the app is killed between the link and the removal of
the trash copy, the file exists twice and the next open puts the trash copy back; nothing is lost.
