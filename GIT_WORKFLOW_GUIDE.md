# Git Workflow Guide for Multi-Contributor Projects

## Understanding Your Current State

**Current Status:**
- You have **staged changes** (2 files modified)
- Your branch may be ahead of origin/main

**Important:** Before handling a pull request, you should either:
1. Commit your current changes, OR
2. Stash them temporarily

---

## Pull Request vs Merge Request

**They are the same thing!**
- **Pull Request (PR)** = GitHub terminology
- **Merge Request (MR)** = GitLab terminology
- Both refer to a request to merge code from one branch into another

---

## Step-by-Step: Reviewing and Merging a Pull Request

### Step 1: Handle Your Current Work First

**Option A: Commit your changes**
```bash
# Review what you've staged
git diff --staged

# If you're happy with the changes, commit them
git commit -m "Your commit message describing the changes"

# Push to your branch
git push origin main
```

**Option B: Stash your changes temporarily**
```bash
# Save your work temporarily
git stash push -m "WIP: changes before reviewing PR"

# Later, restore your work:
git stash pop
```

### Step 2: Fetch Latest Changes from Remote

```bash
# Get all the latest information from GitHub
git fetch origin

# See all branches (including PR branches)
git branch -a
```

### Step 3: View the Pull Request

**On GitHub:**
1. Go to your repository on GitHub
2. Click "Pull requests" tab
3. Open the PR you want to review
4. Review the changes in the "Files changed" tab

**From Command Line:**
```bash
# See what branch the PR is from (if you know the branch name)
git fetch origin
git log origin/main..origin/PR_BRANCH_NAME --oneline

# Or checkout the PR branch locally to review
git fetch origin PR_BRANCH_NAME:PR_BRANCH_NAME
git checkout PR_BRANCH_NAME
```

### Step 4: Check for Conflicts (SAFELY)

**Method 1: Try a test merge (doesn't actually merge)**
```bash
# Make sure you're on main and it's up to date
git checkout main
git pull origin main

# Try merging without committing (dry run)
git merge --no-commit --no-ff origin/PR_BRANCH_NAME

# If there are conflicts, you'll see them
# If no conflicts, abort the test merge:
git merge --abort
```

**Method 2: Check what would change**
```bash
# See what commits would be merged
git log main..origin/PR_BRANCH_NAME

# See what files would change
git diff main...origin/PR_BRANCH_NAME --name-status

# See the actual changes
git diff main...origin/PR_BRANCH_NAME
```

### Step 5: Review the Code

**Best Practices:**
1. Read through all changed files
2. Check for:
   - Code quality and style
   - Potential bugs
   - Missing error handling
   - Test coverage
   - Documentation updates
3. Test the changes if possible (checkout the branch and test)

**Test the PR locally:**
```bash
# Checkout the PR branch
git fetch origin
git checkout -b review-PR origin/PR_BRANCH_NAME

# Test the code, build, run tests, etc.
# When done reviewing:
git checkout main
git branch -D review-PR  # Delete local review branch
```

### Step 6: Merge the Pull Request

**Option A: Merge via GitHub (RECOMMENDED)**
1. On GitHub, go to the PR page
2. Review all changes
3. Click "Merge pull request"
4. Choose merge type:
   - **Create a merge commit** - preserves full history
   - **Squash and merge** - combines all commits into one
   - **Rebase and merge** - linear history (use carefully)
4. Click "Confirm merge"
5. Pull the changes locally:
   ```bash
   git pull origin main
   ```

**Option B: Merge via Command Line**
```bash
# Make sure you're on main and up to date
git checkout main
git pull origin main

# Merge the PR branch
git merge origin/PR_BRANCH_NAME

# If there are conflicts, Git will tell you:
# - Files with conflicts will be marked
# - You'll need to resolve them manually
# - Then: git add <resolved-files>
# - Then: git commit

# Push the merged changes
git push origin main
```

### Step 7: Handle Conflicts (If Any)

**If conflicts occur:**

1. **Git will mark conflicted files:**
   ```
   <<<<<<< HEAD
   Your current code
   =======
   Code from the PR
   >>>>>>> PR_BRANCH_NAME
   ```

2. **Resolve conflicts:**
   - Open each conflicted file
   - Choose which code to keep (or combine both)
   - Remove the conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`)

3. **Mark conflicts as resolved:**
   ```bash
   # After editing conflicted files
   git add <resolved-file-1>
   git add <resolved-file-2>

   # Complete the merge
   git commit
   ```

4. **Verify the merge:**
   ```bash
   # Check that everything looks good
   git log --oneline --graph -10
   git status
   ```

---

## Safe Workflow Checklist

Before merging a PR:
- [ ] Your local work is committed or stashed
- [ ] You've pulled the latest main branch
- [ ] You've reviewed all changes in the PR
- [ ] You've tested the PR locally (if possible)
- [ ] You've checked for conflicts (dry run merge)
- [ ] You understand what the PR does
- [ ] All conflicts are resolved (if any)

---

## Common Commands Reference

```bash
# See current status
git status

# See what branch you're on
git branch

# Fetch latest from remote (doesn't change your files)
git fetch origin

# Pull latest changes (fetches + merges)
git pull origin main

# See commits that would be merged
git log main..origin/PR_BRANCH_NAME

# See file changes
git diff main...origin/PR_BRANCH_NAME --name-status

# See detailed changes
git diff main...origin/PR_BRANCH_NAME

# Test merge (safe, can abort)
git merge --no-commit --no-ff origin/PR_BRANCH_NAME
git merge --abort  # If you want to cancel

# Actually merge
git merge origin/PR_BRANCH_NAME

# Push your changes
git push origin main
```

---

## Preventing Accidental Deletions

**Git is designed to preserve work!** Here's why it's safe:

1. **Nothing is truly deleted** - Git keeps history
2. **You can always recover** - `git reflog` shows all actions
3. **Branches are preserved** - Merging doesn't delete the PR branch
4. **Conflicts are explicit** - Git won't silently overwrite your work

**If you make a mistake:**
```bash
# See recent actions
git reflog

# Undo last commit (keeps changes)
git reset --soft HEAD~1

# Undo last commit (discards changes - be careful!)
git reset --hard HEAD~1

# Recover a deleted branch
git reflog
git checkout -b recovered-branch <commit-hash>
```

---

## Best Practices

1. **Always pull before merging** - `git pull origin main` first
2. **Review PRs thoroughly** - Don't merge blindly
3. **Test locally when possible** - Checkout the branch and test
4. **Use GitHub's merge button** - It's safer and creates a record
5. **Communicate with your collaborator** - Ask questions if unsure
6. **Keep commits meaningful** - Write clear commit messages
7. **Don't force push to main** - This can cause problems for others

---

## Next Steps for Your Current Situation

1. **First, handle your staged changes:**
   ```bash
   git diff --staged  # Review what you have
   git commit -m "Your message"  # Or stash them
   ```

2. **Then fetch and check the PR:**
   ```bash
   git fetch origin
   # Find out the PR branch name from GitHub
   ```

3. **Review the PR safely:**
   ```bash
   git log main..origin/PR_BRANCH_NAME
   git diff main...origin/PR_BRANCH_NAME
   ```

4. **Merge when ready:**
   - Use GitHub's interface (recommended), OR
   - Merge via command line if you prefer
