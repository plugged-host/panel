CREATE TABLE "domains" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"name" varchar(253) NOT NULL,
	"zone_id" varchar(64),
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "server_subdomains" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"server_uuid" uuid NOT NULL,
	"domain_uuid" uuid NOT NULL,
	"name" varchar(63) NOT NULL,
	"record_type" varchar(8) NOT NULL,
	"records" jsonb DEFAULT '{}' NOT NULL,
	"token_cost" integer DEFAULT 0 NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

ALTER TABLE "servers" ADD COLUMN "subdomain_limit" integer;
CREATE UNIQUE INDEX "domains_name_idx" ON "domains" ("name");
CREATE INDEX "server_subdomains_server_uuid_idx" ON "server_subdomains" ("server_uuid");
CREATE INDEX "server_subdomains_domain_uuid_idx" ON "server_subdomains" ("domain_uuid");
CREATE UNIQUE INDEX "server_subdomains_domain_uuid_name_idx" ON "server_subdomains" ("domain_uuid","name");
ALTER TABLE "server_subdomains" ADD CONSTRAINT "server_subdomains_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;
ALTER TABLE "server_subdomains" ADD CONSTRAINT "server_subdomains_domain_uuid_domains_uuid_fkey" FOREIGN KEY ("domain_uuid") REFERENCES "domains"("uuid");