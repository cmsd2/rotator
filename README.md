# Rotator

A lambda for rotating secrets using AWS Secrets Manager.
Supports some resource types that AWS SM doesn't support out of the box.

Supported resources:

1. IAM Service Specific Credentials

## Setup

1. Build this lambda package and create the lambda in your account
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
