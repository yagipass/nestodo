UPDATE `todos` SET `completed_at` = NULL, `updated_at` = datetime('now')
WHERE `completed_at` IS NOT NULL AND `id` IN (
	WITH RECURSIVE `open_below`(`id`) AS (
		SELECT `parent_id` FROM `todos` WHERE `completed_at` IS NULL AND `parent_id` IS NOT NULL
		UNION
		SELECT `todo`.`parent_id` FROM `todos` AS `todo` JOIN `open_below` ON `todo`.`id` = `open_below`.`id` WHERE `todo`.`parent_id` IS NOT NULL
	)
	SELECT `id` FROM `open_below`
);
