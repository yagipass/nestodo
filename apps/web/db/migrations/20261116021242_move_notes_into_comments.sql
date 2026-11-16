-- void:allow-destructive
CREATE TABLE `todo_comments` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`user_id` text NOT NULL,
	`todo_id` integer NOT NULL,
	`body` text NOT NULL,
	`via` text NOT NULL,
	`created_at` text DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')) NOT NULL,
	`edited_at` text
);
--> statement-breakpoint
CREATE INDEX `todo_comments_user_id_todo_id_idx` ON `todo_comments` (`user_id`,`todo_id`,`created_at`);--> statement-breakpoint
CREATE TABLE `trashed_todos` (
	`id` integer PRIMARY KEY NOT NULL,
	`user_id` text NOT NULL,
	`batch` integer NOT NULL,
	`parent_id` integer,
	`position` integer NOT NULL,
	`title` text NOT NULL,
	`collapsed` integer NOT NULL,
	`completed_at` text,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	`trashed_at` text DEFAULT (datetime('now')) NOT NULL
);
--> statement-breakpoint
CREATE INDEX `trashed_todos_user_id_batch_idx` ON `trashed_todos` (`user_id`,`batch`);--> statement-breakpoint
CREATE INDEX `todo_events_user_id_todo_id_idx` ON `todo_events` (`user_id`,`todo_id`);--> statement-breakpoint
INSERT INTO `todo_comments` (`user_id`, `todo_id`, `body`, `via`, `created_at`)
SELECT `user_id`, `id`, `note`, 'app', `created_at` FROM `todos` WHERE trim(`note`, char(32, 9, 10, 13)) <> '' ORDER BY `id`;--> statement-breakpoint
INSERT INTO `todo_comments` (`user_id`, `todo_id`, `body`, `via`, `created_at`)
SELECT `user_id`, `id`, `note`, 'app', `created_at` FROM `archived_todos` WHERE trim(`note`, char(32, 9, 10, 13)) <> '' ORDER BY `id`;--> statement-breakpoint
ALTER TABLE `archived_todos` DROP COLUMN `note`;--> statement-breakpoint
ALTER TABLE `todos` DROP COLUMN `note`;--> statement-breakpoint
DROP TRIGGER `todo_events_created`;--> statement-breakpoint
CREATE TRIGGER `todo_events_created` AFTER INSERT ON `todos`
WHEN NOT EXISTS (SELECT 1 FROM `trashed_todos` WHERE `id` = NEW.`id`)
BEGIN
	INSERT INTO `todo_events` (`user_id`, `todo_id`, `kind`, `at`) VALUES (NEW.`user_id`, NEW.`id`, 'created', strftime('%Y-%m-%d %H:%M:%f', 'now'));
END;
--> statement-breakpoint
DROP TRIGGER `todo_events_completed`;--> statement-breakpoint
CREATE TRIGGER `todo_events_completed` AFTER UPDATE OF `completed_at` ON `todos`
WHEN OLD.`completed_at` IS NULL AND NEW.`completed_at` IS NOT NULL
BEGIN
	INSERT INTO `todo_events` (`user_id`, `todo_id`, `kind`, `at`) VALUES (NEW.`user_id`, NEW.`id`, 'completed', strftime('%Y-%m-%d %H:%M:%f', 'now'));
END;
--> statement-breakpoint
DROP TRIGGER `todo_events_reopened`;--> statement-breakpoint
CREATE TRIGGER `todo_events_reopened` AFTER UPDATE OF `completed_at` ON `todos`
WHEN OLD.`completed_at` IS NOT NULL AND NEW.`completed_at` IS NULL
BEGIN
	INSERT INTO `todo_events` (`user_id`, `todo_id`, `kind`, `at`) VALUES (NEW.`user_id`, NEW.`id`, 'reopened', strftime('%Y-%m-%d %H:%M:%f', 'now'));
END;