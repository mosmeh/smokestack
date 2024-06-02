# Summary

This is a design document for a new tool smokestack.

The purpose of smokestack is to provide a centralized place for managing and sharing "operations", which are some kind of procedures that can be performed on systems, services, applications, or other components.

The operations can be described in any format:

- Markdown documents, which can be one-time manual operations or runbooks
- Shell scripts
- GitHub PR implementing a new feature
  - The operation is deploying the new code.
- GitHub PR modifying Ansible playbooks
  - The operation is running ansible-playbook with the modified playbooks.

smokestack itself does not manage the content of the operations: it only provides a way to

- Manage metadata of the operations, such as the name, purpose, affected components, and the operator of the operation, etc.
- Notify interested parties about the plan and progress of the operations.
- Coordinate the execution of the operations with other operations or people.

# Motivation

Coordinations and approvals of operations are certainly possible with a combination of the existing tools, such as:

- Chat messages
- JIRA tickets
- Confluence pages
- GitHub PRs

However, using tools not designed for the purpose and mixing different tools in different teams can lead to:

- Lack of a central place to see all operations
  - Operations are scattered in different tools and repositories. When searching for an operation, one has to search in multiple places.
- Lack of clear approval process
  - Some tools such as GitHub PRs have an approval feature, but not all tools have it, making it unclear whether an operation is approved or not. This poses a compliance risk.
- Lack of unified announcement process and notifications
  - When an operation is planned, started, paused, or completed, it is important to notify relevant parties. However, the notification process is usually not unified.
  - Notifications can be scattered in different tools and channels.
  - One may not be interested in all notifications in a channel. Noisy notifications make it hard to notice important notifications.
- Lack of scheduling and visibility of future operations
  - It tends to be difficult to know about operations that are planned to start in the future.
  - To avoid conflicts and surprises, it is important to let relevant parties know about future operations.
- Lack of clear dependency management and mutual exclusion
  - It is not clear which operations are blocked by which operations, and which operations cannot be started at the same time.
- Lack of clear history
  - When an operation is started, paused, or completed is important information when investigating an incident. However, this information is usually not clearly recorded.
  - Operation history is one of the most important pieces of information to keep for compliance.
- Ununified format of operation descriptions
  - Different teams use different formats to describe operations. This makes it hard to search for operations and understand the content of the operations.
  - Machine-readable format is usually not provided, making it hard to analyze past operations.

# Features

smokestack is a server-client application. The server is a web application providing a RESTful API for managing the operations. During the PoC phase, the client is a command line tool that interacts with the server. In the future, web frontends and other clients can be developed.

smokestack provides the following features:

- Register operations
- List ongoing, finished, and future operations
- Announce the start, pause, and completion of operations
- Subscribe to updates of operations targeting specific components or tagged with specific tags
- Ensures that dependencies and/or mutual exclusions between operations are satisfied
- Ensures that operations are approved by relevant parties before starting

# Concepts

## Operation

See the above summary for the definition.

Operations can be created with one of the following methods:

- From an operation description
  - Users can create an operation by filling out a form in the CLI tool.
  - Users can also create a template for frequently used operations.
- By importing from a GitHub PR
  - An operation can be automatically created from the description and the content of a GitHub PR.
  - As a generalization, users can create "importers" for other sources, such as GitLab, Jira, or even a local file system. Importers are just local scripts that read the source and produce operation descriptions.
- Via REST API
  - Automation tools can create operations via the REST API.

Operations have key 5W1H attributes:

- What: title
- Why: purpose
- Where: components
- Who: operators
- When: schedule
- How: url

## Operation ID

A sequential integer that uniquely identifies an operation.

## Title

A short name of the operation identifying what the operation does.

## Purpose

A short description of why the operation is needed.

## URL

A link to a web page containing enough information about how the operation is performed.
This can be a link to a script, a runbook, a GitHub PR, or any other source of information.

The philosophy is that without a description of the exact procedure of the operation, announcements of the operation are meaningless. However, we have limited space to describe the operation in the operation description. Thus, the URL is required to provide more detailed information.

## Status

A state of an operation.

- planned: The operation is planned but not started yet.
- in_progress: The operation is in progress.
- paused: The operation is paused.
- completed: The operation was finished successfully.
- aborted: The operation was finished unsuccessfully.
- canceled: The operation was canceled before starting.

State transitions:

| From        | To          | Trigger command |
|-------------|-------------|-----------------|
| planned     | in_progress | start           |
| in_progress | paused      | pause           |
| paused      | in_progress | start           |
| in_progress | completed   | complete        |
| in_progress | aborted     | abort           |
| planned     | canceled    | cancel          |

## User

A person or a system that interacts with smokestack.

## User group

A group of users. A user can belong to multiple user groups.

## Operator

A user who performs an operation. There can be multiple operators for a single operation.

An operator can be a system user, in which case the operation is automated (e.g. periodic maintenance triggered by a cron job). Note that when a human operator triggers the automation, the human operator should be set to the operator of the operation, so that clear responsibility is assigned.

## Approval

Certain operations might require careful review before execution. In this case, the operation is not allowed to start until it is approved.

Tags or components can be configured to automatically require approval by a specific user or users in a specific user group when an operation with the tag or components is created. The required number of approvals can be configured.

Operations imported from GitHub PRs automatically synchronize approvals with the PR.

## Schedule and actual start/end time

An operation can have a scheduled start and end time.

This is just one of metadata and does not enforce the actual start and end time of the operation. The operation is not automatically started at the scheduled time, nor is it prohibited from starting before the scheduled time.

Operations without a scheduled time are considered to be planned to start sometime in the future, and once started, it is considered to continue indefinitely until finished.

Once an operation is started or finished, the start and end time of the operation are updated to reflect the actual start and end time. If an operation continues after the scheduled end time, the end time is updated to be undefined.

## Component

A system, service, application, or other scopes that operations affect. An operation can have multiple target components.

## Tag

A label that can be attached to an operation.

Tags can be used to group operations, and notify subscribers who are interested in operations with certain tags. Multiple tags can be attached to a single operation.

## Operation dependency

An operation can depend on other operations. An operation cannot be started until all its dependencies are completed.

When a user adds a dependency to an operation, the user automatically subscribes to the dependent operation, allowing the user to receive notifications when their operations are unblocked.

## Exclusive lock

An operation can exclusively lock components. When an operation is started, it locks its lock target components. Other operations targeting the locked components are blocked until the operation that locked the component is finished. Conversely, an operation that tries to lock a component cannot be started if any operation targeting it is in progress.

By default, target components are not locked, meaning any number of operations can work on the same component at the same time unless someone locks the component.

Locks are automatically released when an operation is finished.

## Annotation

Annotations are arbitrary key-value pairs that can be attached to an operation. Annotations can be used to store additional information about the operation, such as:

- Ticket number
- Template used to create the operation
- Level of urgency (e.g. high, medium, low)
- Worst-case impact (e.g. downtime, data loss)

## History

All operations and their status changes are stored in a history. The history can be queried to get logs of operations for specific time ranges, operators, components, tags, etc.

## Subscription

A subscription is a way to receive notifications about operations.

A subscription can be created for a specific operation, or all operations with a certain component or tag.

## Notification

There are two types of notifications:

### To subscribers

When an operation is started, paused, or completed, all subscribers are notified. This can be sent via email or Slack or watched with the CLI tool.

### To systems

smokestack can be configured to send notifications to other systems when specific events for specific components or tags occur.

This can be used, for example, to build a Slack bot that sends a Slack message to a channel when an operation targeting a specific component is started:

```
# in #foo-ops channel
@smokestack subscribe --component foo --status in_progress
```

# CLI

```
Usage: smokestack <COMMAND>

Commands:
  create     Create a new operation
  show       Show an operation
  list       List operations
  edit       Edit an operation
  start      Start an operation
  pause      Pause an operation
  complete   Finish an operation successfully
  abort      Finish an operation unsuccessfully
  cancel     Cancel an operation before starting
  subscribe  Subscribe to an operation, component, or tag
  watch      Watch notifications
  approve    Approve an operation
  component  Manage components
  tag        Manage tags
  group      Manage user groups
```

# Operation description

An operation description is a YAML document used when showing, creating, or editing an operation.

| Field       | Description                                            | Type                  | Note            |
|-------------|--------------------------------------------------------|-----------------------|-----------------|
| id          | Unique identifier of the operation                     | integer               |                 |
| title       | Short name of the operation                            | string                | free form       |
| purpose     | Purpose of the operation                               | string                | free form       |
| url         | URL of the operation procedure                         | string                | URL             |
| components  | List of components affected by this operation          | [string]              | component names |
| locks       | List of components that this operation locks           | [string]              | component names |
| tags        | List of tags this operation is associated with         | [string]              | tag names       |
| depends_on  | List of operations that this operation depends on      | [integer]             | operation IDs   |
| starts_at   | Scheduled or actual start time                         | timestamp             |                 |
| ends_at     | Scheduled or actual end time                           | timestamp             |                 |
| operators   | List of users who perform the operation                | [string]              | user names      |
| approved_by | List of users who approved the operation               | [string]              | user names      |
| status      | Status of the operation                                | string                | status          |
| annotations | Arbitrary key-value pairs                              | {string: string}      |                 |

# Workflow

This section describes the typical workflow of smokestack by pretending to be a documentation of the CLI tool.

## Register and perform an operation

### Register an operation

```sh
$ smokestack create
# Editor opens with the following empty operation description
title:
purpose:
url:
components: []
tags: []
depends_on: []
locks: []
annotations: {}

# Fill out the template and save
title: Update kernel of machines for foo service
purpose: To mitigate the security vulnerability CVE-1234-5678
url: https://ghe.example.com/sre/foo-ops/pull/1234
components: [foo]
tags: [security]
depends_on: []
locks: []
annotations: {}

Created operation 123: Update kernel of machines for foo service
```

### Show your operation

```sh
$ smokestack show 123
title: Update kernel of machines for foo service
purpose: To mitigate the security vulnerability CVE-1234-5678
url: https://ghe.example.com/sre/foo-ops/pull/1234
components: [foo]
tags: [security]
operators: [alice]
status: planned

# By default, recent, present, and future operations in your subscription are listed.
# You are automatically subscribed to your own operations.
$ smokestack list
id   status       title                                      start             end
---  -----------  -----------------------------------------  ----------------  ----------------
123  planned      Update kernel of machines for foo service
114  in_progress  Load test xyz service                      2024-01-01 12:34
110  completed    Set up monitoring for qux service          2023-12-30 10:45  2023-12-30 11:45
```

### Start an operation

```sh
$ smokestack start 123
Started operation 123: Update kernel of machines for foo service
```

### Complete an operation

```sh
$ smokestack complete 123
Completed operation 123: Update kernel of machines for foo service
```

## Subscribe to operations

### Subscribe to operations with specific component or tag

```sh
$ smokestack subscribe --component foo
Subscribed to operations targeting foo

$ smokestack subscribe --tag security
Subscribed to operations tagged with security

$ smokestack subscribe --list
components:
  - foo
tags:
  - security
```

### Watch notifications

```sh
# tail -f style
$ smokestack watch
time              operation  status       title
----------------  ---------  -----------  -----
2024-01-01 16:11  123        in_progress  Update kernel of machines for foo service
2024-01-01 18:11  123        completed    Update kernel of machines for foo service
2024-01-02 11:48  124        planned      Deploy new version of foo and bar services
2024-01-02 14:26  126        planned      Connect foo service to bar service
2024-01-02 15:01  124        in_progress  Deploy new version of foo and bar services
2024-01-02 15:14  124        paused       Deploy new version of foo and bar services
2024-01-02 15:37  124        in_progress  Deploy new version of foo and bar services
2024-01-02 16:52  124        completed    Deploy new version of foo and bar services
2024-01-02 17:03  126        in_progress  Connect foo service to bar service
2024-01-02 17:05  126        completed    Connect foo service to bar service
```

## Import an operation from a GitHub PR

### Create an importer for the foo-ops repository

Let's say we have a repository `foo-ops` which contains operations as GitHub PRs.

Create a file `~/.smokestack/importers/foo-ops.sh`

```sh
#!/bin/bash

# Parameters are passed as environment variables named SMOKESTACK_PARAM_{NAME}
# This importer expects SMOKESTACK_PARAM_URL to be a URL of a GitHub PR in the foo-ops repository.

# If this importer can handle this import, it should print the operation description to stdout and exit with 0.
# Otherwise, exit with 125 to let smokestack try other importers.
[[ "$SMOKESTACK_PARAM_URL" =~ ^https://ghe.example.com/sre/foo-ops/pull/[0-9]+$ ]] || exit 125

# Fetch the GitHub PR with the URL and extract information
gh pr view "$SMOKESTACK_PARAM_URL" --json title,files,labels,assignees | jq ... # (details omitted)

cat <<EOS
title: $TITLE
purpose: $PURPOSE
url: $SMOKESTACK_ARG_URL
components: $COMPONENTS
tags: $TAGS
operators: $OPERATORS
annotations:
  ticket: $TICKET
EOS
```

Usually, the importer script should be committed to the repository so that other people can use it by:

```sh
cp /path/to/foo-ops/smokestack-importer.sh ~/.smokestack/importers/foo-ops.sh
```

### Import an operation from a GitHub PR for implementing a new feature

```sh
# Importer is automatically detected from the parameters
$ smokestack create -p url=https://ghe.example.com/sre/foo-ops/pull/4567
Created operation 124: Deploy new version of foo and bar services

# Or specify the importer explicitly when ambiguous
$ smokestack create --importer foo-ops -p url=https://ghe.example.com/sre/foo-ops/pull/4567
```

### View the operation

```sh
$ smokestack show 124
title: Deploy new version of foo and bar services
purpose: To enable new features and fix bugs
url: https://ghe.example.com/dev/monorepo/pull/5678
components: [foo, bar]
tags: [deploy, dev]
operators: [bob]
status: planned
annotations:
  ticket: DEV-1234
```

### Modify the operation

```sh
$ smokestack edit 124 --start 15:00 --end 17:00 --lock foo

$ smokestack edit 124
# Editor opens to edit the operation description

$ smokestack show 124
title: Deploy new version of foo and bar services
purpose: To enable new features and fix bugs
url: https://ghe.example.com/dev/monorepo/pull/5678
components: [foo, bar]
locks: [foo]
tags: [deploy, dev]
starts_at: 2024-01-02 15:00
ends_at: 2024-01-02 17:00
operators: [bob]
status: planned
annotations:
  ticket: DEV-1234
  service_impact: possible downtime in case of rollback
```

## Create an operation from a template

### Create a template

Create a file `~/.smokestack/templates/deploy_all_services.yaml.j2`

```sh
title: Deploy all services
purpose: To deploy new versions of all services
url: https://ghe.example.com/sre/monorepo/pull/{{ pr_number }}
components: [foo, bar, baz]
tags: [deploy]
```

### Create an operation from the template

```sh
$ smokestack create --template deploy_all_services -p pr_number=5678
# Editor opens with the operation description pre-filled with the template

Created operation 125: Deploy all services
```

## Operations with conflicts

### Query operations targeting foo service

```sh
$ smokestack list --component foo
id  title                                      start			      end              status
--- ------------------------------------------ ---------------- ---------------- ---------
124 Deploy new version of foo and bar services 2024-01-02 15:00 2024-01-02 17:00 planned
123 Update kernel of machines for foo service  2024-01-01 16:11 2024-01-01 18:11 completed
```

### Register operation with dependencies

```sh
$ smokestack create
title: Connect foo service to bar service
purpose: To enable new features
url: https://ghe.example.com/sre/feature-flags/pull/1234
components: [foo, bar]
tags: [feature]
depends_on: [124]

Created operation 126: Connect foo service to bar service
```

### Try to set a schedule that conflicts with another operation

```sh
$ smokestack edit 126 --start 16:00 --end 18:00
Operation 126 cannot be scheduled because it cannot meet the dependency on operation 124
```

### Try to start the operation

```sh
$ smokestack start 126
Operation 126 cannot start because dependent operation 124 is ongoing
```

### Try again after completing the dependency

```sh
$ smokestack complete 124
Completed operation 124: Deploy new version of foo and bar services

$ smokestack start 126
Operation 126 cannot start because operation 124 exclusive-locks bar
```

### Try again after completing the operation that locks the component

```sh
$ smokestack complete 124
Completed operation 124: Deploy new version of foo and bar services

$ smokestack start 126
Started operation 126: Connect foo service to bar service
```

## Create new tags, components, and user groups

### Create a new tag

```sh
$ smokestack tag create
name: feature
description: Operations related to new features

Created tag feature

$ smokestack tag list
name     description              requires_approval_by required_approvals
-------- ---------------------    -------------------- ------------------
deploy   Deployments
security Security enhancements
feature  Rolling out new features
```

### Create a new component

```sh
$ smokestack component create
name: bar
description: bar service
url: https://confluence.example.com/display/dev/bar
owners: [alice, bob]

Created component bar
```

### Create a new user group

```sh
$ smokestack group create
name: sre
description: Site Reliability Engineers

Created group sre

$ smokestack group add sre alice
$ smokestack group add sre bob

$ smokestack group show sre
name: sre
description: Site Reliability Engineers
members:
  - alice
  - bob
```

## Approve an operation

### Configure component to require approval from a specific user group

```sh
$ smokestack component edit foo --requires-approval-by sre --required-approvals 2

$ smokestack component show foo
name: foo
description: foo service
requires_approval_by: sre
required_approvals: 2
```

### Register an operation that requires approval

```sh
$ smokestack create
title: Migrate foo service to new data center
purpose: To improve latency
url: https://ghe.example.com/sre/foo-ops/pull/2345
components: [foo]
tags: [migration]

Created operation 127: Migrate foo service to new data center
```

### Try to start the operation

```sh
$ smokestack start 127
Operation 127 cannot start because it does not have enough approvals required by the component foo
```

### Approve the operation

```sh
# As alice
$ smokestack approve 127
Approved operation 127: Migrate foo service to new data center

# As bob
$ smokestack approve 127
Approved operation 127: Migrate foo service to new data center
```

### Start the operation

```sh
$ smokestack show 127
title: Migrate foo service to new data center
purpose: To improve latency
url: https://ghe.example.com/sre/foo-ops/pull/2345
components: [foo]
tags: [migration]
operators: [charlie]
approved_by: [alice, bob]
status: planned

$ smokestack start 127
Started operation 127: Migrate foo service to new data center
```
