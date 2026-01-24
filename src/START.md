gbiv is a cli for managing git worktrees in a standardized way. It standardizes your worktrees to being 7 ever-present folders (8 when you include the main folder / branch): each with the name of a color from the rainbow.
This has several benefits:
- Prevents stale folders - You don't end up with 100 folders for features that you're not actually working on
- Persistent environments - You don’t have to reinstall dependencies from scratch or copy config files every time you work on something new. Can also bookmark the folders and get to them quickly.
- Lowers cognitive load - You're never going to be working on more than 7 tasks in parallel.

I need you to implement the "init" command. This command:

- Must be ran from the parent folder of a git repository you're trying to initialize
    - This is so we can create a directory structure like the following for the user:

project/
├─ main/
├─ red/
├─ orange/
├─ yellow/
├─ green/
├─ blue/
├─ indigo/
├─ violet/

Where each folder has a subfolder with the name of the parent project (This is nice because then you see the project name in IDE title bars instead of seeing something like "red")

ex:

project/
├─ main/
│  ├─ project/

The init takes the name of the folder you're trying to initialize as a required argument

The folder being initialized must:

- be a git repo
- Have at least 1 commit (required for worktrees)
- Check if the user already has branches named after any of the ROYGBIV colors. If so, they'll have to delete so that the worktree add commands don't cause issues

If validation checks succeed it proceeds:

- Move the target folder temporarily to that it doesn't conflict with the next operations
- Recreate the target folder
- create the folder ./<target>/main
- Move the original folder into main with it's original name restored: ./<target>/main/<target> (This folder is called "main" even if the repo uses "master" as the default branch name)
- Create ./<target>/<color>/<target> for each of the ROYGBIV colors.
- Find the main branch's name and store for next operation
- From the <target>/main/<target> folder, run `git worktree add <color> ../../<color>/<target> MAIN_BRANCH`
