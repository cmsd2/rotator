import * as iam from 'aws-cdk-lib/aws-iam';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as logs from 'aws-cdk-lib/aws-logs';
import * as codedeploy from 'aws-cdk-lib/aws-codedeploy';
import { Construct } from 'constructs';

export interface LambdaProps {
    hash: string | undefined,
    aliasName: string,
    code: lambda.Code,
    handler: string,
    memorySize: number,
    runtime: lambda.Runtime,
    deploymentConfig: codedeploy.ILambdaDeploymentConfig,
    environment?: { [key: string]: string },
    logGroup?: logs.ILogGroup,
}

export interface LambdaResult {
    role: iam.Role,
    lambda: lambda.Function,
}

export function lambda_deployment(construct: Construct, id: string, props: LambdaProps): LambdaResult {
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
      logGroup: props.logGroup,
      // Auto lets Lambda roll out runtime patches, including ca-certificates updates
      runtimeManagementMode: lambda.RuntimeManagementMode.AUTO,
    });

    const version = new lambda.Version(fn, 'Version' + props.hash, {
      lambda: fn,
    });

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