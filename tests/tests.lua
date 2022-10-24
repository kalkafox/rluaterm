local json = require("json")

-- Sort a table of numbers from lowest to highest
local function sortNumbers(t)
    table.sort(t, function(a, b) return a < b end)
end

math.randomseed(os.time())

local function run_tests()
    log.info("Running tests...")

    -- http library
    log.info("HTTP Library")
    r = http.get("https://jsonplaceholder.typicode.com/posts")
    log.info("Got " .. r.status .. "!")

    r_data = json.decode(r.text)
    log.info("Got " .. #r_data .. " posts!")

    -- color library
    log.info("Color Library")
    log.info(color.red("This is red!"))
    log.info(color.green("This is green!"))
    log.info(color.blue("This is blue!"))
    log.info(color.yellow("This is yellow!"))
    log.info(color.magenta("This is magenta!"))
    log.info(color.cyan("This is cyan!"))
    log.info(color.white("This is white!"))
    log.info(color.black("This is black!"))
    log.info(color.bold("This is bold!"))
    log.info(color.italic("This is italic!"))
    log.info(color.underline("This is underlined!"))
    log.info(color.reverse("This is reversed!"))

    for k, v in pairs(r_data) do
        log.info(color.red(v.title))
        log.info(color.blue(v.body))
        log.info(color.green(v.id))
    end

    local num = {}
    
    log.info(color.green("Generating a random list of numbers... approximately 1,000,000 numbers..."))

    for i = 1, 1000000 do
        num[i] = math.random()
    end
    
    log.info("Sorting...")
    sortNumbers(num)

    log.info("Reversing table...")

    -- Reverse the table
    for i = 1, math.floor(#num / 2) do
        num[i], num[#num - i + 1] = num[#num - i + 1], num[i]
    end

    log.info("Done!")
end

run_tests()

