# Rotator

A lambda for rotating secrets using AWS Secrets Manager.
Supports some resource types that AWS SM doesn't support out of the box.

Supported resources:

1. IAM Service Specific Credentials

## Setup

1. Build this lambda package and create the lambda in your account optionally using the provided `infra` AWS CDK project
2. Add tags to your secret so that Rotator knows how to rotate it. See below
3. Set up rotation for a AWS SM secret using this lambda

## Configuration

Each secret needs to be tagged with some configuration settings that define how Rotator attempts to rotate it.

The following table defines the supported tags:

| Tag Name             | Example Values                     |
| -------------------- | ---------------------------------- |
| rotator:resourceType | ServiceSpecificCredential          |
| rotator:userName     | Bob                                |
| rotator:serviceName  | codecommit.amazonaws.com           |

Using these example settings, rotator will create or update a CodeCommit credential for the user Bob.

## Infrastructure

The [infra](./infra) directory contains two cloudformation stacks defined using AWS CDK.

 * The `rotator` stack deploys the packaged lambda and necessary supporting roles.
 * The `rotator-test` stack is a test stack for trying out secret rotation with an IAM user and a CodeCommit repo.

Every effort has been made to limit the scope of permissions provided to various components.

Please review the code before deploying into your own account.
