CREATE TABLE "status_components" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"kind" varchar(16) NOT NULL,
	"node_uuid" uuid,
	"label" varchar(1020) NOT NULL,
	"published" boolean DEFAULT true NOT NULL,
	"sort" integer DEFAULT 0 NOT NULL,
	"status" varchar(32) DEFAULT 'unknown' NOT NULL,
	"failure_count" integer DEFAULT 0 NOT NULL,
	"healthy_count" integer DEFAULT 0 NOT NULL,
	"pressure_count" integer DEFAULT 0 NOT NULL,
	"last_metrics" jsonb,
	"last_reported" timestamp,
	"checked" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "status_daily_uptime" (
	"component_uuid" uuid,
	"date" date,
	"total_checks" integer DEFAULT 0 NOT NULL,
	"operational_checks" integer DEFAULT 0 NOT NULL,
	"worst_status" varchar(32) DEFAULT 'unknown' NOT NULL,
	CONSTRAINT "status_daily_uptime_pk" PRIMARY KEY("component_uuid","date")
);

CREATE TABLE "status_incident_updates" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"incident_uuid" uuid NOT NULL,
	"status" varchar(32) NOT NULL,
	"message" text NOT NULL,
	"created_by" uuid,
	"published" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "status_incidents" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"title" varchar(1020) NOT NULL,
	"impact" varchar(32) DEFAULT 'minor' NOT NULL,
	"status" varchar(32) DEFAULT 'investigating' NOT NULL,
	"components" uuid[] DEFAULT '{}'::uuid[] NOT NULL,
	"created_by" uuid,
	"started" timestamp DEFAULT now() NOT NULL,
	"resolved" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "status_overrides" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"component_uuid" uuid,
	"status" varchar(32) NOT NULL,
	"note" text,
	"starts" timestamp,
	"ends" timestamp,
	"created_by" uuid,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE UNIQUE INDEX "status_components_node_uuid_idx" ON "status_components" ("node_uuid");
CREATE UNIQUE INDEX "status_components_endpoint_kind_idx" ON "status_components" ("kind") WHERE node_uuid IS NULL;
CREATE INDEX "status_components_published_sort_idx" ON "status_components" ("published","sort");
CREATE INDEX "status_daily_uptime_date_idx" ON "status_daily_uptime" ("date");
CREATE INDEX "status_incident_updates_incident_uuid_published_idx" ON "status_incident_updates" ("incident_uuid","published");
CREATE INDEX "status_incidents_status_started_idx" ON "status_incidents" ("status","started");
CREATE INDEX "status_overrides_component_uuid_idx" ON "status_overrides" ("component_uuid");
ALTER TABLE "status_components" ADD CONSTRAINT "status_components_node_uuid_nodes_uuid_fkey" FOREIGN KEY ("node_uuid") REFERENCES "nodes"("uuid") ON DELETE CASCADE;
ALTER TABLE "status_daily_uptime" ADD CONSTRAINT "status_daily_uptime_component_uuid_status_components_uuid_fkey" FOREIGN KEY ("component_uuid") REFERENCES "status_components"("uuid") ON DELETE CASCADE;
ALTER TABLE "status_incident_updates" ADD CONSTRAINT "status_incident_updates_UJdqcnJaVIhC_fkey" FOREIGN KEY ("incident_uuid") REFERENCES "status_incidents"("uuid") ON DELETE CASCADE;
ALTER TABLE "status_incident_updates" ADD CONSTRAINT "status_incident_updates_created_by_users_uuid_fkey" FOREIGN KEY ("created_by") REFERENCES "users"("uuid") ON DELETE SET NULL;
ALTER TABLE "status_incidents" ADD CONSTRAINT "status_incidents_created_by_users_uuid_fkey" FOREIGN KEY ("created_by") REFERENCES "users"("uuid") ON DELETE SET NULL;
ALTER TABLE "status_overrides" ADD CONSTRAINT "status_overrides_component_uuid_status_components_uuid_fkey" FOREIGN KEY ("component_uuid") REFERENCES "status_components"("uuid") ON DELETE CASCADE;
ALTER TABLE "status_overrides" ADD CONSTRAINT "status_overrides_created_by_users_uuid_fkey" FOREIGN KEY ("created_by") REFERENCES "users"("uuid") ON DELETE SET NULL;