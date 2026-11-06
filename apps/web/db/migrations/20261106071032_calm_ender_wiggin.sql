CREATE TABLE `analytics_consents` (
	`user_id` text PRIMARY KEY NOT NULL
);
--> statement-breakpoint
CREATE TABLE `archived_todos` (
	`id` integer PRIMARY KEY NOT NULL,
	`user_id` text NOT NULL,
	`path` text NOT NULL,
	`title` text NOT NULL,
	`note` text NOT NULL,
	`created_at` text NOT NULL,
	`completed_at` text NOT NULL
);
--> statement-breakpoint
CREATE INDEX `archived_todos_user_id_completed_at_idx` ON `archived_todos` (`user_id`,`completed_at`);--> statement-breakpoint
CREATE TABLE `todo_events` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`user_id` text NOT NULL,
	`todo_id` integer NOT NULL,
	`kind` text NOT NULL,
	`at` text DEFAULT (datetime('now')) NOT NULL
);
--> statement-breakpoint
CREATE INDEX `todo_events_user_id_at_idx` ON `todo_events` (`user_id`,`at`);--> statement-breakpoint
CREATE TABLE `todos` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`user_id` text NOT NULL,
	`parent_id` integer,
	`position` integer DEFAULT 0 NOT NULL,
	`title` text NOT NULL,
	`note` text DEFAULT '' NOT NULL,
	`collapsed` integer DEFAULT false NOT NULL,
	`completed_at` text,
	`created_at` text DEFAULT (datetime('now')) NOT NULL,
	`updated_at` text DEFAULT (datetime('now')) NOT NULL
);
--> statement-breakpoint
CREATE INDEX `todos_user_id_idx` ON `todos` (`user_id`);--> statement-breakpoint
CREATE TABLE `webhooks` (
	`user_id` text PRIMARY KEY NOT NULL,
	`target` text NOT NULL,
	`has_token` integer NOT NULL,
	`header_names` text NOT NULL,
	`project` text NOT NULL,
	`last_sent_at` text,
	`last_error` text
);
--> statement-breakpoint
CREATE TRIGGER `todo_events_created` AFTER INSERT ON `todos`
BEGIN
	INSERT INTO `todo_events` (`user_id`, `todo_id`, `kind`) VALUES (NEW.`user_id`, NEW.`id`, 'created');
END;
--> statement-breakpoint
CREATE TRIGGER `todo_events_completed` AFTER UPDATE OF `completed_at` ON `todos`
WHEN OLD.`completed_at` IS NULL AND NEW.`completed_at` IS NOT NULL
BEGIN
	INSERT INTO `todo_events` (`user_id`, `todo_id`, `kind`) VALUES (NEW.`user_id`, NEW.`id`, 'completed');
END;
--> statement-breakpoint
CREATE TRIGGER `todo_events_reopened` AFTER UPDATE OF `completed_at` ON `todos`
WHEN OLD.`completed_at` IS NOT NULL AND NEW.`completed_at` IS NULL
BEGIN
	INSERT INTO `todo_events` (`user_id`, `todo_id`, `kind`) VALUES (NEW.`user_id`, NEW.`id`, 'reopened');
END;
--> statement-breakpoint
CREATE TRIGGER `todo_events_deleted` AFTER DELETE ON `todos`
WHEN OLD.`completed_at` IS NULL
BEGIN
	INSERT INTO `todo_events` (`user_id`, `todo_id`, `kind`) VALUES (OLD.`user_id`, OLD.`id`, 'deleted');
END;
