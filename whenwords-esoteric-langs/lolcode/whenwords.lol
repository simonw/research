BTW whenwords library in LOLCODE
BTW Human-friendly time formatting and parsing
BTW Transpiles to JavaScript via lolcode npm package
BTW Functions: timeago, duration

HAI 1.3

BTW ========================================
BTW timeago function
BTW Convert timestamp difference to human string
BTW ========================================

HOW DUZ I timeago YR timestamp AN YR reference
    I HAS A diff ITZ DIFF OF reference AN timestamp
    I HAS A absDiff
    diff SMALLR THAN 0
    O RLY?
        YA RLY
            absDiff R DIFF OF 0 AN diff
        NO WAI
            absDiff R diff
    OIC
    I HAS A isFuture ITZ FAIL
    diff SMALLR THAN 0
    O RLY?
        YA RLY
            isFuture R WIN
    OIC

    BTW Just now: 0-44 seconds
    absDiff SMALLR THAN 45
    O RLY?
        YA RLY
            FOUND YR "just now"
    OIC

    BTW 1 minute: 45-89 seconds
    absDiff SMALLR THAN 90
    O RLY?
        YA RLY
            BOTH SAEM isFuture AN WIN
            O RLY?
                YA RLY
                    FOUND YR "in 1 minute"
                NO WAI
                    FOUND YR "1 minute ago"
            OIC
    OIC

    BTW n minutes: 90 seconds to 2699 seconds (44 min 59 sec)
    absDiff SMALLR THAN 2700
    O RLY?
        YA RLY
            I HAS A mins ITZ QUOSHUNT OF SUM OF absDiff AN 30 AN 60
            mins R DIFF OF mins AN MOD OF mins AN 1
            BOTH SAEM isFuture AN WIN
            O RLY?
                YA RLY
                    FOUND YR SMOOSH "in " AN mins AN " minutes" MKAY
                NO WAI
                    FOUND YR SMOOSH mins AN " minutes ago" MKAY
            OIC
    OIC

    BTW 1 hour: 45-89 minutes (2700-5399 sec)
    absDiff SMALLR THAN 5400
    O RLY?
        YA RLY
            BOTH SAEM isFuture AN WIN
            O RLY?
                YA RLY
                    FOUND YR "in 1 hour"
                NO WAI
                    FOUND YR "1 hour ago"
            OIC
    OIC

    BTW n hours: 90 min to 21 hrs (5400-79199 sec)
    absDiff SMALLR THAN 79200
    O RLY?
        YA RLY
            I HAS A hrs ITZ QUOSHUNT OF SUM OF absDiff AN 1800 AN 3600
            hrs R DIFF OF hrs AN MOD OF hrs AN 1
            BOTH SAEM hrs AN 1
            O RLY?
                YA RLY
                    BOTH SAEM isFuture AN WIN
                    O RLY?
                        YA RLY
                            FOUND YR "in 1 hour"
                        NO WAI
                            FOUND YR "1 hour ago"
                    OIC
                NO WAI
                    BOTH SAEM isFuture AN WIN
                    O RLY?
                        YA RLY
                            FOUND YR SMOOSH "in " AN hrs AN " hours" MKAY
                        NO WAI
                            FOUND YR SMOOSH hrs AN " hours ago" MKAY
                    OIC
            OIC
    OIC

    BTW 1 day: 22-35 hrs (79200-129599 sec)
    absDiff SMALLR THAN 129600
    O RLY?
        YA RLY
            BOTH SAEM isFuture AN WIN
            O RLY?
                YA RLY
                    FOUND YR "in 1 day"
                NO WAI
                    FOUND YR "1 day ago"
            OIC
    OIC

    BTW n days: 36 hrs to 25 days (129600-2246399 sec)
    absDiff SMALLR THAN 2246400
    O RLY?
        YA RLY
            I HAS A days ITZ QUOSHUNT OF SUM OF absDiff AN 43200 AN 86400
            days R DIFF OF days AN MOD OF days AN 1
            BOTH SAEM days AN 1
            O RLY?
                YA RLY
                    BOTH SAEM isFuture AN WIN
                    O RLY?
                        YA RLY
                            FOUND YR "in 1 day"
                        NO WAI
                            FOUND YR "1 day ago"
                    OIC
                NO WAI
                    BOTH SAEM isFuture AN WIN
                    O RLY?
                        YA RLY
                            FOUND YR SMOOSH "in " AN days AN " days" MKAY
                        NO WAI
                            FOUND YR SMOOSH days AN " days ago" MKAY
                    OIC
            OIC
    OIC

    BTW 1 month: 26-45 days (2246400-3887999 sec)
    absDiff SMALLR THAN 3888000
    O RLY?
        YA RLY
            BOTH SAEM isFuture AN WIN
            O RLY?
                YA RLY
                    FOUND YR "in 1 month"
                NO WAI
                    FOUND YR "1 month ago"
            OIC
    OIC

    BTW n months: 46-319 days (3888000-27647999 sec)
    absDiff SMALLR THAN 27648000
    O RLY?
        YA RLY
            I HAS A months ITZ QUOSHUNT OF SUM OF absDiff AN 1296000 AN 2592000
            months R DIFF OF months AN MOD OF months AN 1
            BOTH SAEM months AN 1
            O RLY?
                YA RLY
                    BOTH SAEM isFuture AN WIN
                    O RLY?
                        YA RLY
                            FOUND YR "in 1 month"
                        NO WAI
                            FOUND YR "1 month ago"
                    OIC
                NO WAI
                    BOTH SAEM isFuture AN WIN
                    O RLY?
                        YA RLY
                            FOUND YR SMOOSH "in " AN months AN " months" MKAY
                        NO WAI
                            FOUND YR SMOOSH months AN " months ago" MKAY
                    OIC
            OIC
    OIC

    BTW 1 year: 320-547 days (27648000-47303999 sec)
    absDiff SMALLR THAN 47304000
    O RLY?
        YA RLY
            BOTH SAEM isFuture AN WIN
            O RLY?
                YA RLY
                    FOUND YR "in 1 year"
                NO WAI
                    FOUND YR "1 year ago"
            OIC
    OIC

    BTW n years: 548+ days
    I HAS A years ITZ QUOSHUNT OF SUM OF absDiff AN 15768000 AN 31536000
    years R DIFF OF years AN MOD OF years AN 1
    BOTH SAEM years AN 0
    O RLY?
        YA RLY
            years R 1
    OIC
    BOTH SAEM years AN 1
    O RLY?
        YA RLY
            BOTH SAEM isFuture AN WIN
            O RLY?
                YA RLY
                    FOUND YR "in 1 year"
                NO WAI
                    FOUND YR "1 year ago"
            OIC
        NO WAI
            BOTH SAEM isFuture AN WIN
            O RLY?
                YA RLY
                    FOUND YR SMOOSH "in " AN years AN " years" MKAY
                NO WAI
                    FOUND YR SMOOSH years AN " years ago" MKAY
            OIC
    OIC
IF U SAY SO

BTW ========================================
BTW duration function
BTW Format seconds as human-readable duration
BTW ========================================

HOW DUZ I duration YR seconds AN YR compact AN YR maxUnits
    BTW Set default values using conditional
    BOTH SAEM compact AN NOOB
    O RLY?
        YA RLY
            compact R FAIL
    OIC
    BOTH SAEM maxUnits AN NOOB
    O RLY?
        YA RLY
            maxUnits R 2
    OIC

    BTW Handle zero
    BOTH SAEM seconds AN 0
    O RLY?
        YA RLY
            BOTH SAEM compact AN WIN
            O RLY?
                YA RLY
                    FOUND YR "0s"
                NO WAI
                    FOUND YR "0 seconds"
            OIC
    OIC

    I HAS A remaining ITZ seconds
    I HAS A parts ITZ ""
    I HAS A unitCount ITZ 0

    BTW Years (365 days = 31536000 seconds)
    I HAS A years ITZ QUOSHUNT OF remaining AN 31536000
    years R DIFF OF years AN MOD OF years AN 1
    remaining R MOD OF remaining AN 31536000
    DIFFRINT years AN 0
    O RLY?
        YA RLY
            unitCount SMALLR THAN maxUnits
            O RLY?
                YA RLY
                    BOTH SAEM compact AN WIN
                    O RLY?
                        YA RLY
                            DIFFRINT parts AN ""
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN " " MKAY
                            OIC
                            parts R SMOOSH parts AN years AN "y" MKAY
                        NO WAI
                            DIFFRINT parts AN ""
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN ", " MKAY
                            OIC
                            BOTH SAEM years AN 1
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN "1 year" MKAY
                                NO WAI
                                    parts R SMOOSH parts AN years AN " years" MKAY
                            OIC
                    OIC
                    unitCount R SUM OF unitCount AN 1
            OIC
    OIC

    BTW Months (30 days = 2592000 seconds)
    I HAS A months ITZ QUOSHUNT OF remaining AN 2592000
    months R DIFF OF months AN MOD OF months AN 1
    remaining R MOD OF remaining AN 2592000
    DIFFRINT months AN 0
    O RLY?
        YA RLY
            unitCount SMALLR THAN maxUnits
            O RLY?
                YA RLY
                    BOTH SAEM compact AN WIN
                    O RLY?
                        YA RLY
                            DIFFRINT parts AN ""
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN " " MKAY
                            OIC
                            parts R SMOOSH parts AN months AN "mo" MKAY
                        NO WAI
                            DIFFRINT parts AN ""
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN ", " MKAY
                            OIC
                            BOTH SAEM months AN 1
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN "1 month" MKAY
                                NO WAI
                                    parts R SMOOSH parts AN months AN " months" MKAY
                            OIC
                    OIC
                    unitCount R SUM OF unitCount AN 1
            OIC
    OIC

    BTW Days (86400 seconds)
    I HAS A days ITZ QUOSHUNT OF remaining AN 86400
    days R DIFF OF days AN MOD OF days AN 1
    remaining R MOD OF remaining AN 86400
    DIFFRINT days AN 0
    O RLY?
        YA RLY
            unitCount SMALLR THAN maxUnits
            O RLY?
                YA RLY
                    BOTH SAEM compact AN WIN
                    O RLY?
                        YA RLY
                            DIFFRINT parts AN ""
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN " " MKAY
                            OIC
                            parts R SMOOSH parts AN days AN "d" MKAY
                        NO WAI
                            DIFFRINT parts AN ""
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN ", " MKAY
                            OIC
                            BOTH SAEM days AN 1
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN "1 day" MKAY
                                NO WAI
                                    parts R SMOOSH parts AN days AN " days" MKAY
                            OIC
                    OIC
                    unitCount R SUM OF unitCount AN 1
            OIC
    OIC

    BTW Hours (3600 seconds)
    I HAS A hours ITZ QUOSHUNT OF remaining AN 3600
    hours R DIFF OF hours AN MOD OF hours AN 1
    remaining R MOD OF remaining AN 3600
    DIFFRINT hours AN 0
    O RLY?
        YA RLY
            unitCount SMALLR THAN maxUnits
            O RLY?
                YA RLY
                    BOTH SAEM compact AN WIN
                    O RLY?
                        YA RLY
                            DIFFRINT parts AN ""
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN " " MKAY
                            OIC
                            parts R SMOOSH parts AN hours AN "h" MKAY
                        NO WAI
                            DIFFRINT parts AN ""
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN ", " MKAY
                            OIC
                            BOTH SAEM hours AN 1
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN "1 hour" MKAY
                                NO WAI
                                    parts R SMOOSH parts AN hours AN " hours" MKAY
                            OIC
                    OIC
                    unitCount R SUM OF unitCount AN 1
            OIC
    OIC

    BTW Minutes (60 seconds)
    I HAS A mins ITZ QUOSHUNT OF remaining AN 60
    mins R DIFF OF mins AN MOD OF mins AN 1
    remaining R MOD OF remaining AN 60
    DIFFRINT mins AN 0
    O RLY?
        YA RLY
            unitCount SMALLR THAN maxUnits
            O RLY?
                YA RLY
                    BOTH SAEM compact AN WIN
                    O RLY?
                        YA RLY
                            DIFFRINT parts AN ""
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN " " MKAY
                            OIC
                            parts R SMOOSH parts AN mins AN "m" MKAY
                        NO WAI
                            DIFFRINT parts AN ""
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN ", " MKAY
                            OIC
                            BOTH SAEM mins AN 1
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN "1 minute" MKAY
                                NO WAI
                                    parts R SMOOSH parts AN mins AN " minutes" MKAY
                            OIC
                    OIC
                    unitCount R SUM OF unitCount AN 1
            OIC
    OIC

    BTW Seconds
    DIFFRINT remaining AN 0
    O RLY?
        YA RLY
            unitCount SMALLR THAN maxUnits
            O RLY?
                YA RLY
                    BOTH SAEM compact AN WIN
                    O RLY?
                        YA RLY
                            DIFFRINT parts AN ""
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN " " MKAY
                            OIC
                            parts R SMOOSH parts AN remaining AN "s" MKAY
                        NO WAI
                            DIFFRINT parts AN ""
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN ", " MKAY
                            OIC
                            BOTH SAEM remaining AN 1
                            O RLY?
                                YA RLY
                                    parts R SMOOSH parts AN "1 second" MKAY
                                NO WAI
                                    parts R SMOOSH parts AN remaining AN " seconds" MKAY
                            OIC
                    OIC
                    unitCount R SUM OF unitCount AN 1
            OIC
    OIC

    BTW If nothing was added return zero
    BOTH SAEM parts AN ""
    O RLY?
        YA RLY
            BOTH SAEM compact AN WIN
            O RLY?
                YA RLY
                    FOUND YR "0s"
                NO WAI
                    FOUND YR "0 seconds"
            OIC
    OIC

    FOUND YR parts
IF U SAY SO

KTHXBYE
