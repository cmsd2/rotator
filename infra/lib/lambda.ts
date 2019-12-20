import * as cdk from '@aws-cdk/core';
import * as iam from '@aws-cdk/aws-iam';
import * as lambda from '@aws-cdk/aws-lambda';
import * as codedeploy from '@aws-cdk/aws-codedeploy';

export interface LambdaProps {
    hash: string | undefined,
    aliasName: string,
    code: any,
    handler: string,
    memorySize: number,
    runtime: any,
    deploymentConfig: codedeploy.ILambdaDeploymentConfig,
    environment?: { [key: string]: string },
}

export interface LambdaResult {
    role: iam.Role,
    lambda: lambda.Function,
}

export function lambda_deployment(construct: cdk.Construct, id: string, props: LambdaProps): LambdaResult {
    if (!props.hash) {
      throw "env var GITHUB_SHA not found";
    }

    let role = new iam.Role(construct, id + '-lambda-role', {
      assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com')
    });

    role.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName("service-role/AWSLambdaBasicExecutionRole"));
    role.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName("AWSXrayWriteOnlyAccess"));

    let fn = new lambda.Function(construct, id + '-function', {
      code: props.code,
      handler: props.handler,
      memorySize: props.memorySize,
      runtime: props.runtime,
      role: role,
      environment: props.environment,
    });

    const version = fn.addVersion(props.hash);

    const alias = new lambda.Alias(construct, id + '-function-alias', {
      aliasName: props.aliasName,
      version: version,
    });

    const app = new codedeploy.LambdaApplication(construct, id + '-lambda-application');

    new codedeploy.LambdaDeploymentGroup(construct, id + '-deployment-group', {
      alias: alias,
      application: app,
      deploymentConfig: props.deploymentConfig,
    });

    return {role, lambda: fn};
}