# Using Kola Hub with an AI assistant

Kola Hub has no AI inside it. It can, if you choose, let an AI assistant you already use (one that supports the
Model Context Protocol, such as Claude Code) look at your files and help you clean them. Everything stays on your
computer. Nothing is sent anywhere by Kola Hub.

## Turn it on

1. Open Kola Hub and go to Settings.
2. Switch on AI access.
3. Open the project folder you want help with. The assistant can only see projects you have opened in Kola Hub.

AI access is off until you switch it on. You can switch it off again at any time, and the next request the
assistant makes is refused.

## Connect an assistant

Kola Hub installs a small program called `kola-hub-mcp`. The assistant starts it when it needs it. For Claude Code,
type this in a terminal (use the real location of the program on your computer):

```
claude mcp add kola-hub -- "C:\Program Files\Kola Hub\kola-hub-mcp.exe"
```

Other assistants have a place to add an MCP server by command. Give it the same program and no extra settings.

## What the assistant can do

- See the names of your open projects and the files in them.
- Read rows of a file, check it for problems, and suggest which spellings are the same thing.
- Show what a cleaning step would change, add or remove steps in the saved list, and reuse the steps of another file.
- Save the cleaned result as a new file next to the original.
- Move a file to Recently deleted, and put it back.

## What it cannot do

- It never changes your original files. Cleaned results are always saved as new files.
- It never sees where your files are kept on the disk, only their names.
- It cannot empty Recently deleted, make or restore backups, forget projects or change your settings.
- It cannot open a folder you have not opened yourself.

A file can contain text that tries to give the assistant orders. Kola Hub tells the assistant that file contents are
only data, but it is still wise to read what the assistant plans to do before you approve it.

## If both are open

If you change the steps of a file in the Kola Hub window and the assistant changes the same file at the same
time, the second save is refused with a message, and nothing is overwritten. Open the file again to see the
newest steps.
