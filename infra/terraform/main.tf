terraform {
  required_version = ">= 1.6"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }

  backend "s3" {
    bucket         = "soroban-loyalty-tfstate"
    key            = "soroban-loyalty/terraform.tfstate"
    region         = "us-east-1"
    dynamodb_table = "soroban-loyalty-tflock"
    encrypt        = true
  }
}

provider "aws" {
  region = var.aws_region
  default_tags {
    tags = {
      project     = "soroban-loyalty"
      environment = var.environment
      owner       = var.owner
      managed_by  = "terraform"
    }
  }
}

module "networking" {
  source      = "./modules/networking"
  environment = var.environment
  vpc_cidr    = var.vpc_cidr
}

module "compute" {
  source            = "./modules/compute"
  environment       = var.environment
  vpc_id            = module.networking.vpc_id
  private_subnet_ids = module.networking.private_subnet_ids
  public_subnet_ids  = module.networking.public_subnet_ids
}

module "database" {
  source             = "./modules/database"
  environment        = var.environment
  vpc_id             = module.networking.vpc_id
  private_subnet_ids = module.networking.private_subnet_ids
  db_password        = var.db_password
}

module "cache" {
  source             = "./modules/cache"
  environment        = var.environment
  vpc_id             = module.networking.vpc_id
  private_subnet_ids = module.networking.private_subnet_ids
}

module "storage" {
  source            = "./modules/storage"
  environment       = var.environment
  oidc_provider_arn = var.oidc_provider_arn
}
